use std::{io, iter, pin::Pin};

use asynchronous_codec::Framed;
use bytes::Bytes;
use futures::{Future, SinkExt, StreamExt};
use libp2p::{
    core::UpgradeInfo,
    futures::{AsyncRead, AsyncWrite},
    InboundUpgrade, OutboundUpgrade, PeerId, StreamProtocol,
};

use crate::proto;

const MAX_MESSAGE_LEN_BYTES: usize = 2048;

const PROTOCOL_NAME: StreamProtocol = StreamProtocol::new("/dog/1.0.0");

#[derive(Clone, Default)]
pub struct DogProtocol {}

impl DogProtocol {
    pub fn new() -> Self {
        Self {}
    }
}

impl UpgradeInfo for DogProtocol {
    type Info = StreamProtocol;
    type InfoIter = iter::Once<Self::Info>;

    fn protocol_info(&self) -> Self::InfoIter {
        iter::once(PROTOCOL_NAME)
    }
}

impl<TSocket> InboundUpgrade<TSocket> for DogProtocol
where
    TSocket: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    type Output = DogRpc;
    type Error = DogError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Output, Self::Error>> + Send>>;

    fn upgrade_inbound(self, socket: TSocket, _: Self::Info) -> Self::Future {
        Box::pin(async move {
            let mut framed = Framed::new(
                socket,
                quick_protobuf_codec::Codec::<proto::RPC>::new(MAX_MESSAGE_LEN_BYTES),
            );

            let rpc = framed
                .next()
                .await
                .ok_or_else(|| DogError::ReadError(io::ErrorKind::UnexpectedEof.into()))?
                .map_err(CodecError)?;

            let mut transactions = Vec::with_capacity(rpc.txs.len());
            for tx in rpc.txs {
                transactions.push(DogTransaction {
                    from: PeerId::from_bytes(&tx.from).map_err(|_| DogError::InvalidPeerId)?,
                    tx_id: tx.tx_id.into(),
                    data: tx.data.into(),
                })
            }

            Ok(DogRpc { transactions })
        })
    }
}

#[derive(thiserror::Error, Debug)]
pub enum DogError {
    /// Error when parsing the `PeerId` in the message.
    #[error("Failed to decode PeerId from message")]
    InvalidPeerId,
    /// Error when decoding the raw buffer into a protobuf.
    #[error("Failed to decode protobuf")]
    ProtobufError(#[from] CodecError),
    /// Error when reading the packet from the socket.
    #[error("Failed to read from socket")]
    ReadError(#[from] io::Error),
}

#[derive(thiserror::Error, Debug)]
#[error(transparent)]
pub struct CodecError(#[from] quick_protobuf_codec::Error);

#[derive(Debug)]
pub struct DogRpc {
    pub transactions: Vec<DogTransaction>,
}

impl UpgradeInfo for DogRpc {
    type Info = StreamProtocol;
    type InfoIter = iter::Once<Self::Info>;

    fn protocol_info(&self) -> Self::InfoIter {
        iter::once(PROTOCOL_NAME)
    }
}

impl<TSocket> OutboundUpgrade<TSocket> for DogRpc
where
    TSocket: AsyncWrite + AsyncRead + Send + Unpin + 'static,
{
    type Output = ();
    type Error = CodecError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Output, Self::Error>> + Send>>;

    fn upgrade_outbound(self, socket: TSocket, _: Self::Info) -> Self::Future {
        Box::pin(async move {
            let mut framed = Framed::new(
                socket,
                quick_protobuf_codec::Codec::<proto::RPC>::new(MAX_MESSAGE_LEN_BYTES),
            );
            framed.send(self.into_rpc()).await?;
            framed.close().await?;
            Ok(())
        })
    }
}

impl DogRpc {
    fn into_rpc(self) -> proto::RPC {
        proto::RPC {
            txs: self
                .transactions
                .into_iter()
                .map(|tx| proto::Tx {
                    from: tx.from.to_bytes().into(),
                    tx_id: tx.tx_id.into(),
                    data: tx.data.to_vec().into(),
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Hash)]
pub struct DogTransaction {
    pub from: PeerId,
    pub tx_id: Vec<u8>,
    pub data: Bytes,
}
