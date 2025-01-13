use prometheus_client::{metrics::counter::Counter, registry::Registry};

pub(crate) struct Metrics {
    /// Number of transactions sent.
    txs_sent_counts: Counter,
    /// Number of bytes sent.
    txs_sent_bytes: Counter,
    /// Number of published transactions.
    txs_sent_published: Counter,

    /// Number of transactions received (without filtering duplicates).
    txs_recv_counts_unfiltered: Counter,
    /// Number of transactions received (after filtering duplicates).
    txs_recv_counts: Counter,
    /// Number of bytes received.
    txs_recv_bytes: Counter,
}

impl Metrics {
    pub(crate) fn new(registry: &mut Registry) -> Self {
        let txs_sent_counts = Counter::default();
        let txs_sent_bytes = Counter::default();
        let txs_sent_published = Counter::default();
        let txs_recv_counts_unfiltered = Counter::default();
        let txs_recv_counts = Counter::default();
        let txs_recv_bytes = Counter::default();

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
            "txs_recv_bytes",
            "Number of bytes received.",
            txs_recv_bytes.clone(),
        );

        Self {
            txs_sent_counts,
            txs_sent_bytes,
            txs_sent_published,
            txs_recv_counts_unfiltered,
            txs_recv_counts,
            txs_recv_bytes,
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
}
