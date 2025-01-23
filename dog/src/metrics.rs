use std::sync::atomic::AtomicU64;

use prometheus_client::{
    metrics::{counter::Counter, gauge::Gauge},
    registry::Registry,
};

pub(crate) struct Metrics {
    /// Number of peers.
    peers_count: Gauge,

    /// Redundancy.
    redundancy: Gauge<f64, AtomicU64>,
    /// Number of have_tx requests sent.
    have_tx_sent_counts: Counter,
    /// Number of reset_route requests sent.
    reset_route_sent_counts: Counter,
    /// Number of disabled routes.
    disabled_routes_count: Gauge,

    /// Number of transactions sent.
    txs_sent_counts: Counter,
    /// Number of bytes sent.
    txs_sent_bytes: Counter,
    /// Number of published transactions.
    txs_sent_published: Counter,
    /// Number of published transactions dropped by the sender.
    published_txs_dropped: Counter,
    /// Number of forwarded transactions dropped by the sender.
    forwarded_txs_dropped: Counter,
    /// Number of transactions that timed out and could not be sent.
    timedout_txs_dropped: Counter,

    /// Number of transactions received (without filtering duplicates).
    txs_recv_counts_unfiltered: Counter,
    /// Number of transactions received (after filtering duplicates).
    txs_recv_counts: Counter,
    /// Number of invalid transactions received.
    txs_invalid_counts: Counter,
    /// Number of bytes received.
    txs_recv_bytes: Counter,

    /// Transactions cache size.
    txs_cache_size: Gauge,
}

impl Metrics {
    pub(crate) fn new(registry: &mut Registry) -> Self {
        let peers_count = Gauge::default();
        let redundancy = Gauge::default();
        let have_tx_sent_counts = Counter::default();
        let reset_route_sent_counts = Counter::default();
        let disabled_routes_count = Gauge::default();
        let txs_sent_counts = Counter::default();
        let txs_sent_bytes = Counter::default();
        let txs_sent_published = Counter::default();
        let published_txs_dropped = Counter::default();
        let forwarded_txs_dropped = Counter::default();
        let timedout_txs_dropped = Counter::default();
        let txs_recv_counts_unfiltered = Counter::default();
        let txs_recv_counts = Counter::default();
        let txs_invalid_counts = Counter::default();
        let txs_recv_bytes = Counter::default();
        let txs_cache_size = Gauge::default();

        registry.register("peers_count", "Number of peers.", peers_count.clone());
        registry.register("redundancy", "Redundancy.", redundancy.clone());
        registry.register(
            "have_tx_sent_counts",
            "Number of have_tx requests sent.",
            have_tx_sent_counts.clone(),
        );
        registry.register(
            "reset_route_sent_counts",
            "Number of reset_route requests sent.",
            reset_route_sent_counts.clone(),
        );
        registry.register(
            "disabled_routes_count",
            "Number of disabled routes.",
            disabled_routes_count.clone(),
        );
        registry.register(
            "txs_sent_counts",
            "Number of transactions sent.",
            txs_sent_counts.clone(),
        );
        registry.register(
            "txs_sent_bytes",
            "Number of bytes sent.",
            txs_sent_bytes.clone(),
        );
        registry.register(
            "txs_sent_published",
            "Number of published transactions.",
            txs_sent_published.clone(),
        );
        registry.register(
            "published_txs_dropped",
            "Number of published transactions dropped by the sender.",
            published_txs_dropped.clone(),
        );
        registry.register(
            "forwarded_txs_dropped",
            "Number of forwarded transactions dropped by the sender.",
            forwarded_txs_dropped.clone(),
        );
        registry.register(
            "timedout_txs_dropped",
            "Number of transactions that timed out and could not be sent.",
            timedout_txs_dropped.clone(),
        );
        registry.register(
            "txs_recv_counts_unfiltered",
            "Number of transactions received (without filtering duplicates).",
            txs_recv_counts_unfiltered.clone(),
        );
        registry.register(
            "txs_recv_counts",
            "Number of transactions received (after filtering duplicates).",
            txs_recv_counts.clone(),
        );
        registry.register(
            "txs_invalid_counts",
            "Number of invalid transactions received.",
            txs_invalid_counts.clone(),
        );
        registry.register(
            "txs_recv_bytes",
            "Number of bytes received.",
            txs_recv_bytes.clone(),
        );
        registry.register(
            "txs_cache_size",
            "Transactions cache size.",
            txs_cache_size.clone(),
        );

        Self {
            peers_count,
            redundancy,
            have_tx_sent_counts,
            reset_route_sent_counts,
            disabled_routes_count,
            txs_sent_counts,
            txs_sent_bytes,
            txs_sent_published,
            published_txs_dropped,
            forwarded_txs_dropped,
            timedout_txs_dropped,
            txs_recv_counts_unfiltered,
            txs_recv_counts,
            txs_invalid_counts,
            txs_recv_bytes,
            txs_cache_size,
        }
    }

    pub(crate) fn inc_peers_count(&mut self) {
        self.peers_count.inc();
    }

    pub(crate) fn dec_peers_count(&mut self) {
        self.peers_count.dec();
    }

    pub(crate) fn set_redundancy(&mut self, redundancy: f64) {
        self.redundancy.set(redundancy);
    }

    pub(crate) fn register_have_tx_sent(&mut self) {
        self.have_tx_sent_counts.inc();
    }

    pub(crate) fn register_reset_route_sent(&mut self) {
        self.reset_route_sent_counts.inc();
    }

    pub(crate) fn set_disabled_routes_count(&mut self, count: usize) {
        if let Ok(count) = count.try_into() {
            self.disabled_routes_count.set(count);
        } else {
            tracing::error!("Failed to set disabled routes count");
        }
    }

    pub(crate) fn tx_sent(&mut self, bytes: usize) {
        self.txs_sent_counts.inc();
        self.txs_sent_bytes.inc_by(bytes as u64);
    }

    pub(crate) fn register_published_tx(&mut self) {
        self.txs_sent_published.inc();
    }

    pub(crate) fn tx_recv_unfiltered(&mut self, bytes: usize) {
        self.txs_recv_counts_unfiltered.inc();
        self.txs_recv_bytes.inc_by(bytes as u64);
    }

    pub(crate) fn tx_recv(&mut self) {
        self.txs_recv_counts.inc();
    }

    pub(crate) fn register_published_tx_dropped(&mut self) {
        self.published_txs_dropped.inc();
    }

    pub(crate) fn register_forwarded_tx_dropped(&mut self) {
        self.forwarded_txs_dropped.inc();
    }

    pub(crate) fn register_timedout_tx_dropped(&mut self) {
        self.timedout_txs_dropped.inc();
    }

    pub(crate) fn register_invalid_tx(&mut self) {
        self.txs_invalid_counts.inc();
    }

    pub(crate) fn set_txs_cache_size(&mut self, size: usize) {
        if let Ok(size) = size.try_into() {
            self.txs_cache_size.set(size);
        } else {
            tracing::error!("Failed to set transactions cache size");
        }
    }
}
