use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use asynchronous_codec::Framed;
use futures::{future::Either, Sink, StreamExt};
use libp2p::{
    core::upgrade::DeniedUpgrade,
    swarm::{
        handler::{
            ConnectionEvent, DialUpgradeError, FullyNegotiatedInbound, FullyNegotiatedOutbound,
        },
        ConnectionHandler, ConnectionHandlerEvent, StreamUpgradeError, SubstreamProtocol,
    },
    Stream,
};
use tracing::{debug, trace, warn};

use crate::{
    error::ValidationError,
    protocol::{DogCodec, ProtocolConfig},
    rpc::Receiver,
    rpc_proto::proto,
    types::{RawTransaction, Rpc, RpcOut},
};

/// The event emitted by the Handler. This informs the behaviour of various events created
/// by the handler.
#[derive(Debug)]
pub enum HandlerEvent {
    /// A Dog RPC transaction has been received. This also contains a list of invalid transactions
    /// (if any) that were received.
    Transaction {
        /// The Dog RPC transaction excluding any invalid transactions.
        rpc: Rpc,
        /// Any invalid transactions that were received in the RPC, along with the associated
        /// validation error.
        invalid_transactions: Vec<(RawTransaction, ValidationError)>,
    },
    /// A transaction to be published was dropped because it could not be sent in time.
    TransactionDropped(RpcOut),
}

/// A message sent from the behaviour to the handler.
#[derive(Debug)]
pub enum HandlerIn {}

/// The maximum number of inbound or outbound substream attempts we allow.
///
/// Dog is supposed to have a single long-lived inbound and outbound substream. On failure, we
/// attempt to recreate the substream. This imposes an upper bound of new substreams before we consider the
/// connection faulty and disable the handler. This also prevents against potential substream spamming.
const MAX_SUBSTREAM_ATTEMPS: usize = 5;

pub enum Handler {
    Enabled(EnabledHandler),
    Disabled(DisabledHandler),
}

/// Protocol Handler that manager a single long-lived substream with a peer.
pub struct EnabledHandler {
    /// Upgrade configuration for the dog protocol.
    listen_protocol: ProtocolConfig,
    /// The single long-lived outbound substream.
    outbound_substream: Option<OutboundSubstreamState>,
    /// The single long-lived inbound substream.
    inbound_substream: Option<InboundSubstreamState>,
    /// The queue of transactions we want to send to the remote peer.
    send_queue: Receiver,
    /// Flag indicating that an outbound substream is being established to prevent
    /// duplicate requests.
    outbound_substream_establishing: bool,
    /// The number of outbound substream attempts.
    outbound_substream_attempts: usize,
    /// The number of inbound substream attempts.
    inbound_substream_attempts: usize,
}

pub enum DisabledHandler {
    /// If the peer doesn't support the dog protocol, we don't immediately disconnect.
    /// Instead, we disable the handler and prevent any incoming or outgoing substreams from being established.
    ProtocolUnsupported,
    /// The maximum number of inbound or outbound substream attempts has been exceeded.
    /// Therefore, the handler has been disabled.
    MaxSubstreamAttempts,
}

/// State of the inbound substream.
enum InboundSubstreamState {
    /// Waiting for a new transaction from the remote. This is the idle state.
    WaitingInput(Framed<Stream, DogCodec>),
    /// The substream is being closed.
    Closing(Framed<Stream, DogCodec>),
    /// An error occurred during processing.
    Poisoned,
}

/// State of the outbound substream.
enum OutboundSubstreamState {
    /// Waiting for a new transaction to send to the remote. This is the idle state.
    WaitingOutput(Framed<Stream, DogCodec>),
    /// Waiting to send a transaction to the remote.
    PendingSend(Framed<Stream, DogCodec>, proto::RPC),
    /// Waiting to flush the outbound substream so that the data arrives to the remote.
    PendingFlush(Framed<Stream, DogCodec>),
    /// An error occurred during processing.
    Poisoned,
}

impl Handler {
    /// Builds a new [`Handler`]
    pub fn new(protocol_config: ProtocolConfig, transaction_queue: Receiver) -> Self {
        Handler::Enabled(EnabledHandler {
            listen_protocol: protocol_config,
            outbound_substream: None,
            inbound_substream: None,
            send_queue: transaction_queue,
            outbound_substream_establishing: false,
            outbound_substream_attempts: 0,
            inbound_substream_attempts: 0,
        })
    }
}

impl EnabledHandler {
    fn on_fully_negotiated_inbound(&mut self, substream: Framed<Stream, DogCodec>) {
        trace!("New inbound substream request");
        self.inbound_substream = Some(InboundSubstreamState::WaitingInput(substream));
    }

    fn on_fully_negotiated_outbound(
        &mut self,
        FullyNegotiatedOutbound { protocol, .. }: FullyNegotiatedOutbound<
            <Handler as ConnectionHandler>::OutboundProtocol,
        >,
    ) {
        let substream = protocol;

        assert!(
            self.outbound_substream.is_none(),
            "Established an outbound substream with one already available"
        );
        self.outbound_substream = Some(OutboundSubstreamState::WaitingOutput(substream));
    }

    fn poll(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<
        ConnectionHandlerEvent<
            <Handler as ConnectionHandler>::OutboundProtocol,
            (),
            <Handler as ConnectionHandler>::ToBehaviour,
        >,
    > {
        if !self.send_queue.poll_is_empty(cx)
            && self.outbound_substream.is_none()
            && !self.outbound_substream_establishing
        {
            self.outbound_substream_establishing = true;
            return Poll::Ready(ConnectionHandlerEvent::OutboundSubstreamRequest {
                protocol: SubstreamProtocol::new(self.listen_protocol.clone(), ()),
            });
        }

        loop {
            match std::mem::replace(
                &mut self.outbound_substream,
                Some(OutboundSubstreamState::Poisoned),
            ) {
                Some(OutboundSubstreamState::WaitingOutput(substream)) => {
                    if let Poll::Ready(Some(mut transaction)) = self.send_queue.poll_next_unpin(cx)
                    {
                        match transaction {
                            RpcOut::Publish {
                                tx: _,
                                ref mut timeout,
                            }
                            | RpcOut::Forward {
                                tx: _,
                                ref mut timeout,
                            } => {
                                if Pin::new(timeout).poll(cx).is_ready() {
                                    self.outbound_substream =
                                        Some(OutboundSubstreamState::WaitingOutput(substream));
                                    return Poll::Ready(ConnectionHandlerEvent::NotifyBehaviour(
                                        HandlerEvent::TransactionDropped(transaction),
                                    ));
                                }
                            }
                            _ => {} // All other transactions are not time-bound.
                        }
                        self.outbound_substream = Some(OutboundSubstreamState::PendingSend(
                            substream,
                            transaction.into_protobuf(),
                        ));
                        continue;
                    }

                    self.outbound_substream =
                        Some(OutboundSubstreamState::WaitingOutput(substream));
                    break;
                }
                Some(OutboundSubstreamState::PendingSend(mut substream, rpc)) => {
                    match Sink::poll_ready(Pin::new(&mut substream), cx) {
                        Poll::Ready(Ok(())) => {
                            match Sink::start_send(Pin::new(&mut substream), rpc) {
                                Ok(()) => {
                                    self.outbound_substream =
                                        Some(OutboundSubstreamState::PendingFlush(substream))
                                }
                                Err(e) => {
                                    debug!("Failed to send transaction on outbound stream: {e}");
                                    self.outbound_substream = None;
                                    break;
                                }
                            }
                        }
                        Poll::Ready(Err(e)) => {
                            debug!("Failed to send transaction on outbound stream: {e}");
                            self.outbound_substream = None;
                            break;
                        }
                        Poll::Pending => {
                            self.outbound_substream =
                                Some(OutboundSubstreamState::PendingSend(substream, rpc));
                            break;
                        }
                    }
                }
                Some(OutboundSubstreamState::PendingFlush(mut substream)) => {
                    match Sink::poll_flush(Pin::new(&mut substream), cx) {
                        Poll::Ready(Ok(())) => {
                            self.outbound_substream =
                                Some(OutboundSubstreamState::WaitingOutput(substream))
                        }
                        Poll::Ready(Err(e)) => {
                            debug!("Failed to flush outbound stream: {e}");
                            self.outbound_substream = None;
                            break;
                        }
                        Poll::Pending => {
                            self.outbound_substream =
                                Some(OutboundSubstreamState::PendingFlush(substream));
                            break;
                        }
                    }
                }
                None => {
                    self.outbound_substream = None;
                    break;
                }
                Some(OutboundSubstreamState::Poisoned) => {
                    unreachable!("Error occurred during outbound stream processing")
                }
            }
        }

        loop {
            match std::mem::replace(
                &mut self.inbound_substream,
                Some(InboundSubstreamState::Poisoned),
            ) {
                Some(InboundSubstreamState::WaitingInput(mut substream)) => {
                    match substream.poll_next_unpin(cx) {
                        Poll::Ready(Some(Ok(transaction))) => {
                            self.inbound_substream =
                                Some(InboundSubstreamState::WaitingInput(substream));
                            return Poll::Ready(ConnectionHandlerEvent::NotifyBehaviour(
                                transaction,
                            ));
                        }
                        Poll::Ready(Some(Err(e))) => {
                            debug!("Failed to read from inbound stream: {e}");
                            self.inbound_substream =
                                Some(InboundSubstreamState::Closing(substream));
                        }
                        Poll::Ready(None) => {
                            debug!("Inbound stream closed by remote");
                            self.inbound_substream =
                                Some(InboundSubstreamState::Closing(substream));
                        }
                        Poll::Pending => {
                            self.inbound_substream =
                                Some(InboundSubstreamState::WaitingInput(substream));
                            break;
                        }
                    }
                }
                Some(InboundSubstreamState::Closing(mut substream)) => {
                    match Sink::poll_close(Pin::new(&mut substream), cx) {
                        Poll::Ready(res) => {
                            if let Err(e) = res {
                                debug!("Inbound substream error while closing: {e}");
                            }
                            self.inbound_substream = None;
                            break;
                        }
                        Poll::Pending => {
                            self.inbound_substream =
                                Some(InboundSubstreamState::Closing(substream));
                            break;
                        }
                    }
                }
                None => {
                    self.inbound_substream = None;
                    break;
                }
                Some(InboundSubstreamState::Poisoned) => {
                    unreachable!("Error occurred during inbound stream processing")
                }
            }
        }

        if let Poll::Ready(Some(rpc)) = self.send_queue.poll_stale(cx) {
            return Poll::Ready(ConnectionHandlerEvent::NotifyBehaviour(
                HandlerEvent::TransactionDropped(rpc),
            ));
        }

        Poll::Pending
    }
}

impl ConnectionHandler for Handler {
    type FromBehaviour = HandlerIn;
    type ToBehaviour = HandlerEvent;
    type InboundOpenInfo = ();
    type InboundProtocol = either::Either<ProtocolConfig, DeniedUpgrade>;
    type OutboundOpenInfo = ();
    type OutboundProtocol = ProtocolConfig;

    fn listen_protocol(&self) -> SubstreamProtocol<Self::InboundProtocol> {
        match self {
            Handler::Enabled(handler) => {
                SubstreamProtocol::new(either::Either::Left(handler.listen_protocol.clone()), ())
            }
            Handler::Disabled(_) => {
                SubstreamProtocol::new(either::Either::Right(DeniedUpgrade), ())
            }
        }
    }

    fn on_behaviour_event(&mut self, message: HandlerIn) {
        match self {
            Handler::Enabled(_) => match message {},
            Handler::Disabled(_) => {
                debug!(?message, "Handler is disabled. Dropping message");
            }
        }
    }

    #[tracing::instrument(level = "trace", name = "ConnectionHandler::poll", skip(self, cx))]
    fn poll(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<ConnectionHandlerEvent<Self::OutboundProtocol, (), Self::ToBehaviour>> {
        match self {
            Handler::Enabled(handler) => handler.poll(cx),
            Handler::Disabled(DisabledHandler::ProtocolUnsupported) => Poll::Pending,
            Handler::Disabled(DisabledHandler::MaxSubstreamAttempts) => Poll::Pending,
        }
    }

    fn on_connection_event(
        &mut self,
        event: ConnectionEvent<Self::InboundProtocol, Self::OutboundProtocol>,
    ) {
        match self {
            Handler::Enabled(handler) => {
                if event.is_inbound() {
                    handler.inbound_substream_attempts += 1;

                    if handler.inbound_substream_attempts == MAX_SUBSTREAM_ATTEMPS {
                        warn!("The maximum number of inbound substream has been exceeded");
                        *self = Handler::Disabled(DisabledHandler::MaxSubstreamAttempts);
                        return;
                    }
                }

                if event.is_outbound() {
                    handler.outbound_substream_establishing = false;

                    handler.outbound_substream_attempts += 1;

                    if handler.outbound_substream_attempts == MAX_SUBSTREAM_ATTEMPS {
                        warn!("The maximum number of outbound substream has been exceeded");
                        *self = Handler::Disabled(DisabledHandler::MaxSubstreamAttempts);
                        return;
                    }
                }

                match event {
                    ConnectionEvent::FullyNegotiatedInbound(FullyNegotiatedInbound {
                        protocol,
                        ..
                    }) => match protocol {
                        Either::Left(protocol) => handler.on_fully_negotiated_inbound(protocol),
                        Either::Right(v) => libp2p_core::util::unreachable(v),
                    },
                    ConnectionEvent::FullyNegotiatedOutbound(fully_negotiated_outbound) => {
                        handler.on_fully_negotiated_outbound(fully_negotiated_outbound);
                    }
                    ConnectionEvent::DialUpgradeError(DialUpgradeError {
                        error: StreamUpgradeError::Timeout,
                        ..
                    }) => {
                        tracing::debug!("Dial upgrade error: Protocol negotiation timeout");
                    }
                    ConnectionEvent::DialUpgradeError(DialUpgradeError {
                        error: StreamUpgradeError::Apply(e),
                        ..
                    }) => libp2p_core::util::unreachable(e),
                    ConnectionEvent::DialUpgradeError(DialUpgradeError {
                        error: StreamUpgradeError::NegotiationFailed,
                        ..
                    }) => {
                        // The protocol is not supported
                        tracing::debug!(
                            "The remote peer does not support gossipsub on this connection"
                        );
                        *self = Handler::Disabled(DisabledHandler::ProtocolUnsupported);
                    }
                    ConnectionEvent::DialUpgradeError(DialUpgradeError {
                        error: StreamUpgradeError::Io(e),
                        ..
                    }) => {
                        tracing::debug!("Protocol negotiation failed: {e}")
                    }
                    _ => {}
                }
            }
            Handler::Disabled(_) => {}
        }
    }
}
