use std::{
    collections::{HashSet, VecDeque},
    hash::DefaultHasher,
    task::{Context, Poll},
};

use bytes::Bytes;
use cuckoofilter::{CuckooError, CuckooFilter};
use libp2p::{
    core::{transport::PortUse, Endpoint},
    swarm::{
        behaviour::ConnectionEstablished, dial_opts::DialOpts, CloseConnection, ConnectionClosed,
        ConnectionDenied, ConnectionId, FromSwarm, NetworkBehaviour, NotifyHandler, OneShotHandler,
        THandler, THandlerInEvent, THandlerOutEvent, ToSwarm,
    },
    Multiaddr, PeerId,
};

use crate::{
    config::DogConfig,
    protocol::{DogProtocol, DogRpc, DogTransaction},
};

pub struct Behaviour {
    events: VecDeque<ToSwarm<DogEvent, DogRpc>>,

    config: DogConfig,

    target_peers: HashSet<PeerId>,

    connected_peers: HashSet<PeerId>,

    received: CuckooFilter<DefaultHasher>,
}

impl Behaviour {
    pub fn new(local_peer_id: PeerId) -> Self {
        Self::from_config(DogConfig::new(local_peer_id))
    }

    pub fn from_config(config: DogConfig) -> Self {
        Self {
            events: VecDeque::new(),
            config,
            target_peers: HashSet::new(),
            connected_peers: HashSet::new(),
            received: CuckooFilter::new(),
        }
    }

    pub fn add_target_peer(&mut self, peer_id: PeerId) {
        self.target_peers.insert(peer_id);
    }

    pub fn publish(&mut self, data: impl Into<Bytes>) {
        let transaction = DogTransaction {
            from: self.config.local_peer_id,
            tx_id: rand::random::<[u8; 20]>().to_vec(), // TODO should be probably a hash of the data
            data: data.into(),
        };

        if let Err(e @ CuckooError::NotEnoughSpace) = self.received.add(&transaction) {
            tracing::warn!(
                "Transaction was added to 'received' Cuckoofilter but some \
                other transaction was removed as a consequence: {}",
                e,
            );
        }

        self.events
            .push_back(ToSwarm::GenerateEvent(DogEvent::Transaction(
                transaction.clone(),
            )));

        for peer_id in self.connected_peers.iter() {
            // TODO: from floodsub, why not iterate over `self.target_peers`?
            if !self.target_peers.contains(peer_id) {
                continue;
            }

            self.events.push_back(ToSwarm::NotifyHandler {
                peer_id: *peer_id,
                handler: libp2p::swarm::NotifyHandler::Any,
                event: DogRpc {
                    transactions: vec![transaction.clone()],
                },
            })
        }
    }

    fn on_connection_established(
        &mut self,
        ConnectionEstablished {
            peer_id,
            other_established,
            ..
        }: ConnectionEstablished,
    ) {
        if other_established > 0 {
            // We only case about the first time a peer connects.
            return;
        }

        self.connected_peers.insert(peer_id);
    }

    fn on_connection_closed(
        &mut self,
        ConnectionClosed {
            peer_id,
            remaining_established,
            ..
        }: ConnectionClosed,
    ) {
        if remaining_established > 0 {
            // We only care about peer disconnections.
            return;
        }

        // We can be disconnected by the remote in case of inactivity for example, so we always
        // try to reconnect.
        if self.target_peers.contains(&peer_id) {
            self.events.push_back(ToSwarm::Dial {
                opts: DialOpts::peer_id(peer_id).build(),
            });
        }
    }
}

impl NetworkBehaviour for Behaviour {
    type ConnectionHandler = OneShotHandler<DogProtocol, DogRpc, InnerMessage>;
    type ToSwarm = DogEvent;

    fn handle_established_inbound_connection(
        &mut self,
        _: ConnectionId,
        _: PeerId,
        _: &Multiaddr,
        _: &Multiaddr,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        Ok(Default::default())
    }

    fn handle_established_outbound_connection(
        &mut self,
        _: ConnectionId,
        _: PeerId,
        _: &Multiaddr,
        _: Endpoint,
        _: PortUse,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        Ok(Default::default())
    }

    fn on_connection_handler_event(
        &mut self,
        propagation_source: PeerId,
        connection_id: ConnectionId,
        event: THandlerOutEvent<Self>,
    ) {
        // We ignore successful sends or timeouts.
        let event = match event {
            Ok(InnerMessage::Rx(event)) => event,
            Ok(InnerMessage::Sent) => return,
            Err(e) => {
                tracing::debug!("Failed to send dog transaction: {e}");
                self.events.push_back(ToSwarm::CloseConnection {
                    peer_id: propagation_source,
                    connection: CloseConnection::One(connection_id),
                });
                return;
            }
        };

        // List of transactions we're going to propagate on the network.
        let mut rpcs_to_dispatch: Vec<(PeerId, DogRpc)> = Vec::new();

        for tx in event.transactions {
            // Use `self.received` to skip the messages that we have already received in the past.
            // Note that this can result in false positives.
            match self.received.test_and_add(&tx) {
                Ok(true) => {}         // Message was added.
                Ok(false) => continue, // Message already existed.
                Err(e @ CuckooError::NotEnoughSpace) => {
                    // Message added, but some other removed.
                    tracing::warn!(
                        "Message was added to 'received' Cuckoofilter but some \
                         other message was removed as a consequence: {}",
                        e,
                    );
                }
            }

            let event = DogEvent::Transaction(tx.clone());
            self.events.push_back(ToSwarm::GenerateEvent(event));

            for peer_id in self.connected_peers.iter() {
                if peer_id == &propagation_source {
                    continue;
                }

                // Peer must be in a communication list.
                if !self.target_peers.contains(peer_id) {
                    continue;
                }

                if let Some(pos) = rpcs_to_dispatch.iter().position(|(p, _)| p == peer_id) {
                    rpcs_to_dispatch[pos].1.transactions.push(tx.clone());
                } else {
                    rpcs_to_dispatch.push((
                        *peer_id,
                        DogRpc {
                            transactions: vec![tx.clone()],
                        },
                    ));
                }
            }
        }

        for (peer_id, rpc) in rpcs_to_dispatch {
            self.events.push_back(ToSwarm::NotifyHandler {
                peer_id,
                handler: NotifyHandler::Any,
                event: rpc,
            });
        }
    }

    #[tracing::instrument(level = "trace", name = "NetworkBehaviour::poll", skip(self))]
    fn poll(&mut self, _: &mut Context<'_>) -> Poll<ToSwarm<Self::ToSwarm, THandlerInEvent<Self>>> {
        if let Some(event) = self.events.pop_front() {
            return Poll::Ready(event);
        }

        Poll::Pending
    }

    fn on_swarm_event(&mut self, event: FromSwarm) {
        match event {
            FromSwarm::ConnectionEstablished(connectioin_established) => {
                self.on_connection_established(connectioin_established);
            }
            FromSwarm::ConnectionClosed(connection_closed) => {
                self.on_connection_closed(connection_closed);
            }
            _ => {}
        }
    }
}

#[derive(Debug)]
pub enum InnerMessage {
    Rx(DogRpc),
    Sent,
}

impl From<DogRpc> for InnerMessage {
    #[inline]
    fn from(rpc: DogRpc) -> Self {
        Self::Rx(rpc)
    }
}

impl From<()> for InnerMessage {
    #[inline]
    fn from(_: ()) -> Self {
        Self::Sent
    }
}

#[derive(Debug)]
pub enum DogEvent {
    Transaction(DogTransaction),
}
