use std::{sync::Arc, time::Duration};

use crate::{
    protocol::ProtocolConfig,
    types::{Transaction, TransactionId},
};

/// The types of transaction validation that can be employed by dog.
#[derive(Debug, Clone)]
pub enum ValidationMode {
    /// This is the default setting. This settings validates all fields of the transaction.
    Strict,
    /// This setting only checks that the author field is valid
    None,
}

/// Configuration parameters that define the performance of the dog network.
#[derive(Clone)]
pub struct Config {
    protocol: ProtocolConfig,
    transaction_id_fn: Arc<dyn Fn(&Transaction) -> TransactionId + Send + Sync + 'static>,
    max_transactions_per_rpc: Option<usize>,
    connection_handler_queue_len: usize,
    cache_time: Duration,
    target_redundancy: f64,
    redundancy_delta_percent: u8,
    redundancy_interval: Duration,
    connection_handler_publish_duration: Duration,
    connection_handler_forward_duration: Duration,
    deliver_own_transactions: bool,
    forward_transactions: bool,
}

impl Config {
    pub(crate) fn protocol_config(&self) -> ProtocolConfig {
        self.protocol.clone()
    }

    /// A user-defined function allowing the user to specify the transaction id of a dog transaction.
    /// The default value is to concatenate the source peer id with a sequence number.
    pub fn transaction_id(&self, tx: &Transaction) -> TransactionId {
        (self.transaction_id_fn)(tx)
    }

    /// The maximum number of transactions we will process in a given RPC. If this is unset, there is
    /// no limit. The default is `None`.
    pub fn max_transactions_per_rpc(&self) -> Option<usize> {
        self.max_transactions_per_rpc
    }

    /// The maximum number of messages a `ConnectionHandler` can buffer. The default is 5000.
    pub fn connection_handler_queue_len(&self) -> usize {
        self.connection_handler_queue_len
    }

    /// The time a transaction id is stored in the cache. The default is 30 seconds.
    pub fn cache_time(&self) -> Duration {
        self.cache_time
    }

    /// The target redundancy for the network. The default is 1.0.
    pub fn target_redundancy(&self) -> f64 {
        self.target_redundancy
    }

    /// The percentage of the target redundancy that the network can deviate from. The default is 10.
    pub fn redundancy_delta_percent(&self) -> u8 {
        self.redundancy_delta_percent
    }

    /// Time between each redundancy adjustment (default is 1 second).
    pub fn redundancy_interval(&self) -> Duration {
        self.redundancy_interval
    }

    /// The maximum byte size for each dog RPC (default is 65536 bytes).
    pub fn max_transmit_size(&self) -> usize {
        self.protocol.max_transmit_size
    }

    /// The duration a transaction to be published can wait to be sent before it is abandoned. The
    /// default is 5 seconds.
    pub fn publish_queue_duration(&self) -> Duration {
        self.connection_handler_publish_duration
    }

    /// The duration a transaction to be forwarded can wait to be sent before it is abandoned. The
    /// default is 1 second.
    pub fn forward_queue_duration(&self) -> Duration {
        self.connection_handler_forward_duration
    }

    /// Whether the node should deliver its own transactions to the user. The default is `false`.
    pub fn deliver_own_transactions(&self) -> bool {
        self.deliver_own_transactions
    }

    /// Whether the node should forward transactions to other peers. The default is `true`.
    /// This is used if a node just wants to be a "client" in the network.
    pub fn forward_transactions(&self) -> bool {
        self.forward_transactions
    }
}

impl Default for Config {
    fn default() -> Self {
        ConfigBuilder::default()
            .build()
            .expect("Default config parameters should be valid parameters")
    }
}

/// The builder struct for constructing a dog configuration.
pub struct ConfigBuilder {
    config: Config,
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self {
            config: Config {
                protocol: ProtocolConfig::default(),
                transaction_id_fn: Arc::new(|tx| {
                    // default transaction id is: source + sequence number
                    let mut from_string = tx.from.to_base58();
                    from_string.push_str(&tx.seqno.to_string());
                    TransactionId::from(from_string)
                }),
                max_transactions_per_rpc: None,
                connection_handler_queue_len: 5000,
                cache_time: Duration::from_secs(30),
                target_redundancy: 1.0,
                redundancy_delta_percent: 10,
                redundancy_interval: Duration::from_secs(1),
                connection_handler_publish_duration: Duration::from_secs(5),
                connection_handler_forward_duration: Duration::from_secs(1),
                deliver_own_transactions: false,
                forward_transactions: true,
            },
        }
    }
}

impl From<Config> for ConfigBuilder {
    fn from(config: Config) -> Self {
        Self { config }
    }
}

impl ConfigBuilder {
    /// A user-defined function allowing the user to specify the transaction id of a dog transaction.
    /// The default value is to concatenate the source peer id with a sequence number.
    pub fn transaction_id_fn<F>(&mut self, transaction_id_fn: F) -> &mut Self
    where
        F: Fn(&Transaction) -> TransactionId + Send + Sync + 'static,
    {
        self.config.transaction_id_fn = Arc::new(transaction_id_fn);
        self
    }

    /// The maximum number of transactions we will process in a given RPC. If this is unset, there is
    /// no limit. The default is `None`.
    pub fn max_transactions_per_rpc(&mut self, max_transactions_per_rpc: usize) -> &mut Self {
        self.config.max_transactions_per_rpc = Some(max_transactions_per_rpc);
        self
    }

    /// The maximum byte size for each dog RPC (default is 65536 bytes).
    pub fn connection_handler_queue_len(
        &mut self,
        connection_handler_queue_len: usize,
    ) -> &mut Self {
        self.config.connection_handler_queue_len = connection_handler_queue_len;
        self
    }

    /// The time a transaction id is stored in the cache. The default is 30 seconds.
    pub fn cache_time(&mut self, cache_time: Duration) -> &mut Self {
        self.config.cache_time = cache_time;
        self
    }

    /// The target redundancy for the network. The default is 1.0.
    pub fn target_redundancy(&mut self, target_redundancy: f64) -> &mut Self {
        self.config.target_redundancy = target_redundancy;
        self
    }

    /// The percentage of the target redundancy that the network can deviate from. The default is 10.
    pub fn redundancy_delta_percent(&mut self, redundancy_delta_percent: u8) -> &mut Self {
        self.config.redundancy_delta_percent = redundancy_delta_percent;
        self
    }

    /// Time between each redundancy adjustment (default is 1 second).
    pub fn redundancy_interval(&mut self, redundancy_interval: Duration) -> &mut Self {
        self.config.redundancy_interval = redundancy_interval;
        self
    }

    /// The maximum byte size for each dog RPC (default is 65536 bytes).
    pub fn max_transmit_size(&mut self, max_transmit_size: usize) -> &mut Self {
        self.config.protocol.max_transmit_size = max_transmit_size;
        self
    }

    /// The duration a transaction to be published can wait to be sent before it is abandoned. The
    /// default is 5 seconds.
    pub fn connection_handler_publish_duration(
        &mut self,
        connection_handler_publish_duration: Duration,
    ) -> &mut Self {
        self.config.connection_handler_publish_duration = connection_handler_publish_duration;
        self
    }

    /// The duration a transaction to be forwarded can wait to be sent before it is abandoned. The
    /// default is 1 second.
    pub fn connection_handler_forward_duration(
        &mut self,
        connection_handler_forward_duration: Duration,
    ) -> &mut Self {
        self.config.connection_handler_forward_duration = connection_handler_forward_duration;
        self
    }

    /// Whether the node should deliver its own transactions to the user. The default is `false`.
    pub fn deliver_own_transactions(&mut self, deliver_own_transactions: bool) -> &mut Self {
        self.config.deliver_own_transactions = deliver_own_transactions;
        self
    }

    /// Whether the node should forward transactions to other peers. The default is `true`.
    /// This is used if a node just wants to be a "client" in the network.
    pub fn forward_transactions(&mut self, forward_transactions: bool) -> &mut Self {
        self.config.forward_transactions = forward_transactions;
        self
    }

    /// Determines the level of validation used when receiving transactions. See [`ValidationMode`]
    /// for the available types. the default is `ValidationMode::Strict`.
    pub fn validation_mode(&mut self, validation_mode: ValidationMode) -> &mut Self {
        self.config.protocol.validation_mode = validation_mode;
        self
    }

    /// Constructs a `Config` from the parameters set in the builder.
    pub fn build(&self) -> Result<Config, &'static str> {
        // TODO: validate config

        Ok(self.config.clone())
    }
}
