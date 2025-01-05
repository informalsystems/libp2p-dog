use std::{future::Future, iter, pin::Pin};

use asynchronous_codec::{Decoder, Encoder, Framed};
use futures::future;
use libp2p::{
    core::UpgradeInfo,
    futures::{AsyncRead, AsyncWrite},
    InboundUpgrade, OutboundUpgrade, PeerId, StreamProtocol,
};
use void::Void;

use crate::{
    config::ValidationMode,
    error::ValidationError,
    handler::HandlerEvent,
    rpc_proto::proto,
    types::{ControlAction, HaveTx, RawTransaction, ResetRoute, Rpc, TransactionId},
};

const DOG_PROTOCOL: &str = "/dog/1.0.0";
const DEFAULT_MAX_TRANSMIT_SIZE: usize = 65536;

/// Implementation of [`InboundUpgrade`] and [`OutboundUpgrade`] for the dog protocol.
#[derive(Debug, Clone)]
pub struct ProtocolConfig {
    /// The dog protocol id to listen on.
    pub(crate) stream_protocol: StreamProtocol,
    /// The maximum transmit size for a packet.
    pub(crate) max_transmit_size: usize,
    /// Determines the level of validation to perform on incoming transactions.
    pub(crate) validation_mode: ValidationMode,
}

impl Default for ProtocolConfig {
    fn default() -> Self {
        Self {
            stream_protocol: StreamProtocol::new(DOG_PROTOCOL),
            max_transmit_size: DEFAULT_MAX_TRANSMIT_SIZE,
            validation_mode: ValidationMode::Strict,
        }
    }
}

impl UpgradeInfo for ProtocolConfig {
    type Info = StreamProtocol;
    type InfoIter = iter::Once<Self::Info>;

    fn protocol_info(&self) -> Self::InfoIter {
        iter::once(self.stream_protocol.clone())
    }
}

impl<TSocket> InboundUpgrade<TSocket> for ProtocolConfig
where
    TSocket: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    type Output = Framed<TSocket, DogCodec>;
    type Error = Void; // TODO: change to Infallible with next rust-libp2p release
    type Future = Pin<Box<dyn Future<Output = Result<Self::Output, Self::Error>> + Send>>;

    fn upgrade_inbound(self, socket: TSocket, _: Self::Info) -> Self::Future {
        Box::pin(future::ok(Framed::new(
            socket,
            DogCodec::new(self.max_transmit_size, self.validation_mode),
        )))
    }
}

impl<TSocket> OutboundUpgrade<TSocket> for ProtocolConfig
where
    TSocket: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    type Output = Framed<TSocket, DogCodec>;
    type Error = Void; // TODO: change to Infallible with next rust-libp2p release
    type Future = Pin<Box<dyn Future<Output = Result<Self::Output, Self::Error>> + Send>>;

    fn upgrade_outbound(self, socket: TSocket, _: Self::Info) -> Self::Future {
        Box::pin(future::ok(Framed::new(
            socket,
            DogCodec::new(self.max_transmit_size, self.validation_mode),
        )))
    }
}

/// Dog codec for the framing
pub struct DogCodec {
    /// Determines the level of validation to perform on incoming transactions.
    validation_mode: ValidationMode,
    /// The codec to handle common encoding/decoding of the protobuf messages.
    codec: quick_protobuf_codec::Codec<proto::RPC>,
}

impl DogCodec {
    pub fn new(max_length: usize, validation_mode: ValidationMode) -> DogCodec {
        let codec = quick_protobuf_codec::Codec::new(max_length);
        DogCodec {
            validation_mode,
            codec,
        }
    }
}

impl Encoder for DogCodec {
    type Item<'a> = proto::RPC;
    type Error = quick_protobuf_codec::Error;

    fn encode(
        &mut self,
        item: Self::Item<'_>,
        dst: &mut bytes::BytesMut,
    ) -> Result<(), Self::Error> {
        self.codec.encode(item, dst)
    }
}

impl Decoder for DogCodec {
    type Item = HandlerEvent;
    type Error = quick_protobuf_codec::Error;

    fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let Some(rpc) = self.codec.decode(src)? else {
            return Ok(None);
        };

        let mut transactions = Vec::with_capacity(rpc.txs.len());
        let mut invalid_transactions = Vec::new();

        for transaction in rpc.txs.into_iter() {
            // TODO: Implement all the validation logic here.
            let mut verify_source = true;

            match self.validation_mode {
                ValidationMode::Strict => {
                    verify_source = true;
                }
                ValidationMode::None => {}
            }

            let source = if verify_source {
                match PeerId::from_bytes(&transaction.from) {
                    Ok(peer_id) => peer_id,
                    Err(_) => {
                        invalid_transactions.push((
                            RawTransaction {
                                from: PeerId::random(),
                                seqno: transaction.seqno,
                                data: transaction.data,
                            },
                            ValidationError::InvalidPeerId,
                        ));
                        continue;
                    }
                }
            } else {
                // TODO: Temporary solution to showcase the validation logic.
                PeerId::random()
            };

            transactions.push(RawTransaction {
                from: source,
                seqno: transaction.seqno,
                data: transaction.data,
            })
        }

        let mut control_msgs = Vec::new();

        if let Some(control) = rpc.control {
            let have_tx_msgs = control.have_tx.into_iter().map(|have_tx| {
                ControlAction::HaveTx(HaveTx {
                    tx_id: TransactionId::from(have_tx.tx_id),
                })
            });

            let reset_route_msgs = control
                .reset_route
                .into_iter()
                .map(|_| ControlAction::ResetRoute(ResetRoute {}));

            control_msgs.extend(have_tx_msgs);
            control_msgs.extend(reset_route_msgs);
        }

        Ok(Some(HandlerEvent::Transaction {
            rpc: Rpc {
                transactions,
                control_msgs,
            },
            invalid_transactions,
        }))
    }
}
