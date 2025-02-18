use std::{
    collections::{HashMap, VecDeque},
    task::Poll,
    time::SystemTime,
};

use futures::FutureExt;
use futures_timer::Delay;
use libp2p::{
    identity::Keypair,
    swarm::{
        behaviour::ConnectionEstablished, ConnectionClosed, FromSwarm, NetworkBehaviour, ToSwarm,
    },
    PeerId,
};
use prometheus_client::registry::Registry;
use quick_protobuf::{MessageWrite, Writer};

use crate::{
    config::Config,
    dog::{Controller, Route, Router},
    error::PublishError,
    handler::{Handler, HandlerEvent, HandlerIn},
    metrics::Metrics,
    protocol::SIGNING_PREFIX,
    rpc::Sender,
    rpc_proto::proto,
    time_cache::DuplicateCache,
    transform::{DataTransform, IdentityTransform},
    types::{
        ControlAction, HaveTx, PeerConnections, RawTransaction, ResetRoute, RpcOut, Transaction,
        TransactionId,
    },
};

/// Determines if published transaction should be signed or not.
#[derive(Debug)]
pub enum TransactionAuthenticity {
    /// Transaction signing is enabled. The author will be the owner of the key and
    /// the sequence number will be linearly increasing.
    Signed(Keypair),
    /// Transaction signing is disabled. The specified [`PeerId`] will be used as the author
    /// of all published transactions. The sequence number will be linearly increasing.
    Author(PeerId),
}

/// Event that can be emitted by the dog behaviour.
#[derive(Debug)]
pub enum Event {
    /// A transaction has been received.
    Transaction {
        /// The peer that forwarded us this transaction.
        propagation_source: PeerId,
        /// The [`TransactionId`] of the transaction. This is the main identifier of the transaction.
        transaction_id: TransactionId,
        /// The transaction itself.
        transaction: Transaction,
    },
    /// The router's routes have been updated.
    RoutingUpdated {
        /// The current disabled routes.
        disabled_routes: Vec<Route>,
    },
}

// A data structure for storing configuration for publishing transactions.
enum PublishConfig {
    Signing {
        keypair: Keypair,
        author: PeerId,
        inline_key: Option<Vec<u8>>,
        last_seqno: SequenceNumber,
    },
    Author {
        author: PeerId,
        last_seqno: SequenceNumber,
    },
}

/// A strictly linearly increasing sequence number.
///
/// We start from the current time as unix timestamp in milliseconds.
#[derive(Debug)]
struct SequenceNumber(u64);

impl SequenceNumber {
    fn new() -> Self {
        let unix_timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("time to be linear")
            .as_nanos();

        Self(unix_timestamp as u64)
    }

    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .checked_add(1)
            .expect("to not exhaust u64 space for sequence numbers");

        self.0
    }
}

impl PublishConfig {
    pub(crate) fn get_own_id(&self) -> PeerId {
        match self {
            Self::Signing { author, .. } => *author,
            Self::Author { author, .. } => *author,
        }
    }
}

impl From<TransactionAuthenticity> for PublishConfig {
    fn from(authenticity: TransactionAuthenticity) -> Self {
        match authenticity {
            TransactionAuthenticity::Signed(keypair) => {
                let public_key = keypair.public();
                let key_enc = public_key.encode_protobuf();
                let key = if key_enc.len() <= 42 {
                    // The public key can be inlined in [`rpc_proto::proto::Transaction::from`], so we
                    // don't include it specifically in the
                    // [`rpc_proto::proto::Transaction::key`] field.
                    None
                } else {
                    // Include the protobuf encoding of the public key in the message.
                    Some(key_enc)
                };

                PublishConfig::Signing {
                    keypair,
                    author: public_key.to_peer_id(),
                    inline_key: key,
                    last_seqno: SequenceNumber::new(),
                }
            }
            TransactionAuthenticity::Author(author) => PublishConfig::Author {
                author,
                last_seqno: SequenceNumber::new(),
            },
        }
    }
}

/// Network behaviour that handles the dog protocol.
///
/// NOTE: Initialisation requires a [`TransactionAuthenticity`]  and [`Config`] instance.
///
/// The DataTransform trait allows applications to optionally add extra encoding/decoding
/// functionality to the underlying transactions. This is intented for custom compression algorithms.
pub struct Behaviour<D = IdentityTransform> {
    config: Config,
    events: VecDeque<ToSwarm<Event, HandlerIn>>,
    publish_config: PublishConfig,
    data_transform: D,
    connected_peers: HashMap<PeerId, PeerConnections>,
    redundancy_interval: Delay,
    redundancy_controller: Controller,
    router: Router,
    cache: DuplicateCache<TransactionId, PeerId>,
    metrics: Option<Metrics>,
}

impl<D> Behaviour<D>
where
    D: DataTransform + Default,
{
    pub fn new(
        authenticity: TransactionAuthenticity,
        config: Config,
    ) -> Result<Self, &'static str> {
        Self::new_with_transform(authenticity, config, None, D::default())
    }

    pub fn new_with_metrics(
        authenticity: TransactionAuthenticity,
        config: Config,
        metrics: &mut Registry,
    ) -> Result<Self, &'static str> {
        Self::new_with_transform(authenticity, config, Some(metrics), D::default())
    }
}

impl<D> Behaviour<D>
where
    D: DataTransform,
{
    pub fn new_with_transform(
        authenticity: TransactionAuthenticity,
        config: Config,
        metrics: Option<&mut Registry>,
        data_transform: D,
    ) -> Result<Self, &'static str> {
        // TODO: validate config

        Ok(Self {
            events: VecDeque::new(),
            publish_config: PublishConfig::from(authenticity),
            data_transform,
            connected_peers: HashMap::new(),
            redundancy_interval: Delay::new(config.redundancy_interval()),
            redundancy_controller: Controller::new(&config),
            router: Router::new(),
            cache: DuplicateCache::new(config.cache_time()),
            config,
            metrics: metrics.map(Metrics::new),
        })
    }
}

impl<D> Behaviour<D>
where
    D: DataTransform + Send + 'static,
{
    pub fn publish(&mut self, data: impl Into<Vec<u8>>) -> Result<TransactionId, PublishError> {
        let data = data.into();

        let transformed_data = self.data_transform.outbound_transform(data.clone())?;

        if transformed_data.len() > self.config.max_transmit_size() {
            return Err(PublishError::TransactionTooLarge);
        }

        let raw_transaction = self.build_raw_transaction(transformed_data)?;

        let transaction = Transaction {
            from: raw_transaction.from,
            seqno: raw_transaction.seqno,
            data,
        };

        let tx_id = self.config.transaction_id(&transaction);

        if self.cache.contains(&tx_id) {
            tracing::warn!(transaction=%tx_id, "Not publishing a transaction that has already been published");
            return Err(PublishError::Duplicate);
        }

        tracing::trace!("Publishing transaction");

        self.cache
            .insert(tx_id.clone(), self.publish_config.get_own_id());

        if let Some(m) = self.metrics.as_mut() {
            m.set_txs_cache_size(self.cache.len());
        }

        if self.config.deliver_own_transactions() {
            self.events
                .push_back(ToSwarm::GenerateEvent(Event::Transaction {
                    propagation_source: self.publish_config.get_own_id(),
                    transaction_id: tx_id.clone(),
                    transaction,
                }));
        }

        let recipient_peers = self.router.filter_valid_routes(
            self.publish_config.get_own_id(),
            self.connected_peers.keys().cloned().collect::<Vec<_>>(),
        );

        let mut publish_failed = true;
        for peer_id in &recipient_peers {
            tracing::trace!(peer=%peer_id, "Sending transaction to peer");
            if self.send_transaction(
                *peer_id,
                RpcOut::Publish {
                    tx: raw_transaction.clone(),
                    timeout: Delay::new(self.config.publish_queue_duration()),
                },
            ) {
                publish_failed = false;
            }
        }

        if recipient_peers.is_empty() {
            return Err(PublishError::InsufficientPeers);
        }

        if publish_failed {
            return Err(PublishError::AllQueuesFull(self.connected_peers.len()));
        }

        tracing::debug!(transaction=%tx_id, "Published transaction");

        if let Some(m) = self.metrics.as_mut() {
            m.register_published_tx();
        }

        Ok(tx_id)
    }

    fn build_raw_transaction(&mut self, data: Vec<u8>) -> Result<RawTransaction, PublishError> {
        match &mut self.publish_config {
            PublishConfig::Signing {
                ref keypair,
                author,
                inline_key,
                last_seqno,
            } => {
                let seqno = last_seqno.next();

                let signature = {
                    let transaction = proto::Transaction {
                        from: author.to_bytes(),
                        seqno,
                        data: data.clone(),
                        // Signature and key fields are not included in the signature
                        signature: vec![],
                        key: vec![],
                    };

                    let mut buf = Vec::with_capacity(transaction.get_size());
                    let mut writer = Writer::new(&mut buf);

                    transaction
                        .write_message(&mut writer)
                        .expect("Encoding to succeed");

                    let mut signature_bytes = SIGNING_PREFIX.to_vec();
                    signature_bytes.extend_from_slice(&buf);
                    keypair.sign(&signature_bytes)?
                };

                Ok(RawTransaction {
                    from: *author,
                    seqno,
                    data,
                    signature: Some(signature.to_vec()),
                    key: inline_key.clone(),
                })
            }
            PublishConfig::Author { author, last_seqno } => {
                let seqno = last_seqno.next();

                Ok(RawTransaction {
                    from: *author,
                    seqno,
                    data,
                    signature: None,
                    key: None,
                })
            }
        }
    }

    /// Returns `true` if the sending was successful, `false` otherwise.
    fn send_transaction(&mut self, peer_id: PeerId, rpc: RpcOut) -> bool {
        if let Some(m) = self.metrics.as_mut() {
            if let RpcOut::Publish { ref tx, .. } | RpcOut::Forward { ref tx, .. } = rpc {
                m.tx_sent(tx.raw_protobuf_len());
            }
        }

        let Some(peer) = &mut self.connected_peers.get_mut(&peer_id) else {
            tracing::error!(peer=%peer_id, "Could not send rpc to connection handler, peer doesn't exist in connected peers list");
            return false;
        };

        match peer.sender.send_transaction(rpc) {
            Ok(()) => true,
            Err(rpc) => {
                // Sending failed because the channel is full.
                tracing::warn!(peer=%peer_id, "Send Queue full. Could not send {}.", rpc);
                false
            }
        }
    }

    /// Returns `true` if the transaction was forwarded, `false` otherwise.
    fn forward_transaction(
        &mut self,
        transaction_id: &TransactionId,
        raw_transaction: RawTransaction,
        propagation_source: &PeerId,
    ) -> bool {
        tracing::debug!(transaction=%transaction_id, "Forwarding transaction");

        let recipient_peers = self.router.filter_valid_routes(
            *propagation_source,
            self.connected_peers
                .keys()
                .filter(|&peer| peer != propagation_source && peer != &raw_transaction.from)
                .cloned()
                .collect::<Vec<_>>(),
        );

        if recipient_peers.is_empty() {
            return false;
        }

        for peer_id in &recipient_peers {
            tracing::trace!(peer=%peer_id, "Forwarding transaction to peer");
            self.send_transaction(
                *peer_id,
                RpcOut::Forward {
                    tx: raw_transaction.clone(),
                    timeout: Delay::new(self.config.forward_queue_duration()),
                },
            );
        }

        tracing::debug!("Completed forwarding transaction");
        true
    }

    fn on_connection_established(
        &mut self,
        ConnectionEstablished { peer_id, .. }: ConnectionEstablished,
    ) {
        tracing::debug!(peer=%peer_id, "New peer connected");

        if let Some(m) = self.metrics.as_mut() {
            m.inc_peers_count();
        }
    }

    fn on_connection_closed(
        &mut self,
        ConnectionClosed {
            peer_id,
            connection_id,
            remaining_established,
            ..
        }: ConnectionClosed,
    ) {
        if remaining_established != 0 {
            if let Some(peer) = self.connected_peers.get_mut(&peer_id) {
                peer.connections.retain(|&id| id != connection_id);
            }
        } else {
            tracing::debug!(peer=%peer_id, "Peer disconnected");

            if !self.router.reset_routes_with_peer(peer_id).is_empty() {
                self.events
                    .push_back(ToSwarm::GenerateEvent(Event::RoutingUpdated {
                        disabled_routes: self.router.get_disabled_routes(),
                    }));

                if let Some(m) = self.metrics.as_mut() {
                    m.set_disabled_routes_count(self.router.get_disabled_routes().len());
                }
            }
            self.connected_peers.remove(&peer_id);
            self.adjust_redundancy();

            if let Some(m) = self.metrics.as_mut() {
                m.dec_peers_count();
            }
        }
    }

    fn handle_received_transaction(
        &mut self,
        raw_transaction: RawTransaction,
        propagation_source: &PeerId,
    ) {
        if let Some(m) = self.metrics.as_mut() {
            m.tx_recv_unfiltered(raw_transaction.raw_protobuf_len());
        }

        let transaction = match self
            .data_transform
            .inbound_transform(raw_transaction.clone())
        {
            Ok(transaction) => transaction,
            Err(e) => {
                tracing::debug!("Invalid transaction. Transform error: {:?}", e);
                self.handle_invalid_transaction(propagation_source, raw_transaction);
                return;
            }
        };

        let tx_id = self.config.transaction_id(&transaction);

        // TODO: validate transaction if needed

        if !self.cache.insert(tx_id.clone(), *propagation_source) {
            tracing::debug!(transaction=%tx_id, "Transaction already received, ignoring");

            if let Some(m) = self.metrics.as_mut() {
                m.set_txs_cache_size(self.cache.len());
            }

            self.redundancy_controller.incr_duplicate_txs_count();

            if self.redundancy_controller.is_have_tx_blocked() {
                return;
            }

            tracing::debug!(peer=%propagation_source, "Sending HaveTx to peer");

            if self.send_transaction(*propagation_source, RpcOut::HaveTx(HaveTx { tx_id })) {
                self.router.register_have_tx_sent(*propagation_source);
                self.redundancy_controller.block_have_tx();

                if let Some(m) = self.metrics.as_mut() {
                    m.register_have_tx_sent();
                }
            }

            return;
        }
        self.redundancy_controller.incr_first_time_txs_count();

        if let Some(m) = self.metrics.as_mut() {
            m.tx_recv();
            m.set_txs_cache_size(self.cache.len());
        }

        tracing::debug!("Deliver received transaction to user");
        self.events
            .push_back(ToSwarm::GenerateEvent(Event::Transaction {
                propagation_source: *propagation_source,
                transaction_id: tx_id.clone(),
                transaction,
            }));

        if self.config.forward_transactions() {
            self.forward_transaction(&tx_id, raw_transaction, propagation_source);
        }
    }

    fn handle_invalid_transaction(
        &mut self,
        _propagation_source: &PeerId,
        _raw_transaction: RawTransaction,
        // rejection_reason: ???
    ) {
        if let Some(m) = self.metrics.as_mut() {
            m.register_invalid_tx();
        }
    }

    fn handle_have_tx(&mut self, tx_ids: Vec<TransactionId>, propagation_source: &PeerId) {
        tracing::debug!(peer=%propagation_source, "Received HaveTx from peer with {} transaction ids", tx_ids.len());

        for tx_id in tx_ids {
            if let Some(source) = self.cache.get(&tx_id) {
                if *source == *propagation_source {
                    continue;
                }
                tracing::debug!(peer=%propagation_source, "Disabling route from {} to peer", source);
                self.router.disable_route(*source, *propagation_source);
            }
        }

        self.events
            .push_back(ToSwarm::GenerateEvent(Event::RoutingUpdated {
                disabled_routes: self.router.get_disabled_routes(),
            }));

        if let Some(m) = self.metrics.as_mut() {
            m.set_disabled_routes_count(self.router.get_disabled_routes().len());
        }
    }

    fn handle_reset_route(&mut self, propagation_source: &PeerId) {
        tracing::debug!(peer=%propagation_source, "Re-enabling a random route to peer");

        match self.router.enable_random_route_to_peer(*propagation_source) {
            Some(route) => {
                tracing::debug!(peer=%propagation_source, "Re-enabled route {} to peer", route);

                self.events
                    .push_back(ToSwarm::GenerateEvent(Event::RoutingUpdated {
                        disabled_routes: self.router.get_disabled_routes(),
                    }));

                if let Some(m) = self.metrics.as_mut() {
                    m.set_disabled_routes_count(self.router.get_disabled_routes().len());
                }
            }
            None => {
                tracing::warn!(peer=%propagation_source, "No route to re-enable to peer");
            }
        }
    }

    fn adjust_redundancy(&mut self) {
        tracing::debug!("Adjusting redundancy");

        let (redundancy, send_reset_route) = self.redundancy_controller.evaluate();
        if send_reset_route {
            tracing::warn!("Redundancy is too low. Sending reset route");

            match self.router.get_random_have_tx_sent_peer() {
                Some(peer_id) => {
                    tracing::trace!(peer=%peer_id, "Sending reset route to peer");
                    if self.send_transaction(peer_id, RpcOut::ResetRoute(ResetRoute {})) {
                        self.router.remove_have_tx_sent(&peer_id);

                        if let Some(m) = self.metrics.as_mut() {
                            m.register_reset_route_sent();
                        }
                    }
                }
                None => {
                    // This should not happen
                    tracing::warn!("No peers to send reset route to");
                }
            };
        }

        if let Some(m) = self.metrics.as_mut() {
            m.set_redundancy(redundancy);
        }

        self.redundancy_controller.reset_counters();
    }
}

impl<D> NetworkBehaviour for Behaviour<D>
where
    D: DataTransform + Send + 'static,
{
    type ConnectionHandler = Handler;
    type ToSwarm = Event;

    fn handle_established_inbound_connection(
        &mut self,
        connection_id: libp2p::swarm::ConnectionId,
        peer_id: PeerId,
        _: &libp2p::Multiaddr,
        _: &libp2p::Multiaddr,
    ) -> Result<libp2p::swarm::THandler<Self>, libp2p::swarm::ConnectionDenied> {
        let connected_peer = self
            .connected_peers
            .entry(peer_id)
            .or_insert(PeerConnections {
                connections: Vec::new(),
                sender: Sender::new(self.config.connection_handler_queue_len()),
            });

        connected_peer.connections.push(connection_id);

        Ok(Handler::new(
            self.config.protocol_config(),
            connected_peer.sender.new_receiver(),
        ))
    }

    fn handle_established_outbound_connection(
        &mut self,
        _connection_id: libp2p::swarm::ConnectionId,
        peer: PeerId,
        _: &libp2p::Multiaddr,
        _: libp2p::core::Endpoint,
        _: libp2p::core::transport::PortUse,
    ) -> Result<libp2p::swarm::THandler<Self>, libp2p::swarm::ConnectionDenied> {
        let connected_peer = self.connected_peers.entry(peer).or_insert(PeerConnections {
            connections: Vec::new(),
            sender: Sender::new(self.config.connection_handler_queue_len()),
        });

        connected_peer.connections.push(_connection_id);

        Ok(Handler::new(
            self.config.protocol_config(),
            connected_peer.sender.new_receiver(),
        ))
    }

    fn on_connection_handler_event(
        &mut self,
        propagation_source: PeerId,
        _connection_id: libp2p::swarm::ConnectionId,
        event: libp2p::swarm::THandlerOutEvent<Self>,
    ) {
        match event {
            HandlerEvent::Transaction {
                rpc,
                invalid_transactions,
            } => {
                // Log the invalid transactions
                for (transaction, validation_error) in invalid_transactions {
                    tracing::warn!(
                        peer=%propagation_source,
                        from=%transaction.from,
                        "Invalid transaction from peer. Reason: {}",
                        validation_error,
                    );
                }

                // Handle transactions
                for (count, raw_transaction) in rpc.transactions.into_iter().enumerate() {
                    if self.config.max_transactions_per_rpc().is_some()
                        && Some(count) >= self.config.max_transactions_per_rpc()
                    {
                        tracing::warn!("Received more transactions than permitted. Ignoring further transactions. Processed: {}", count);
                        break;
                    }
                    self.handle_received_transaction(raw_transaction, &propagation_source);
                }

                // Handle control messages
                let mut have_tx_ids = Vec::new();
                let mut reset_route = false;
                for control_msg in rpc.control_msgs {
                    match control_msg {
                        ControlAction::HaveTx(have_tx) => {
                            have_tx_ids.push(have_tx.tx_id);
                        }
                        ControlAction::ResetRoute(_) => {
                            reset_route = true;
                        }
                    }
                }
                if !have_tx_ids.is_empty() {
                    self.handle_have_tx(have_tx_ids, &propagation_source);
                }
                if reset_route {
                    self.handle_reset_route(&propagation_source);
                }
            }
            HandlerEvent::TransactionDropped(rpc) => {
                tracing::warn!(
                    peer=%propagation_source,
                    "Dropped transaction from peer. Transaction: {}",
                    rpc
                );

                if let Some(m) = self.metrics.as_mut() {
                    match rpc {
                        RpcOut::Publish { .. } => m.register_published_tx_dropped(),
                        RpcOut::Forward { .. } => m.register_forwarded_tx_dropped(),
                        _ => {}
                    }
                    m.register_timedout_tx_dropped();
                }
            }
        }
    }

    #[tracing::instrument(level = "trace", name = "NetworkBehaviour::poll", skip(self, cx))]
    fn poll(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<ToSwarm<Self::ToSwarm, libp2p::swarm::THandlerInEvent<Self>>> {
        if let Some(event) = self.events.pop_front() {
            return Poll::Ready(event);
        }

        // TODO: the check might be first priority and be done before the event check
        if self.redundancy_interval.poll_unpin(cx).is_ready() {
            self.adjust_redundancy();
            self.redundancy_interval
                .reset(self.config.redundancy_interval());
        }

        Poll::Pending
    }

    fn on_swarm_event(&mut self, event: libp2p::swarm::FromSwarm) {
        match event {
            FromSwarm::ConnectionEstablished(connection_established) => {
                self.on_connection_established(connection_established);
            }
            FromSwarm::ConnectionClosed(connection_closed) => {
                self.on_connection_closed(connection_closed);
            }
            _ => {}
        }
    }
}
