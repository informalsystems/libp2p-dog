use std::time::Duration;

use libp2p::{
    gossipsub::{self, IdentTopic},
    identity::Keypair,
    swarm::{behaviour::toggle::Toggle, NetworkBehaviour},
};
use prometheus_client::registry::Registry;

use crate::config::{Config, Protocol};

pub(crate) const GOSSIPSUB_TOPIC_STR: &str = "benchmark";

const MAX_TRANSMIT_SIZE: usize = 4 * 1024 * 1024; // 4 MiB

#[derive(Debug)]
pub(crate) enum NetworkEvent {
    Dog(libp2p_dog::Event),
    Gossipsub(gossipsub::Event),
}

impl From<libp2p_dog::Event> for NetworkEvent {
    fn from(event: libp2p_dog::Event) -> Self {
        Self::Dog(event)
    }
}

impl From<gossipsub::Event> for NetworkEvent {
    fn from(event: gossipsub::Event) -> Self {
        Self::Gossipsub(event)
    }
}

#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "NetworkEvent")]
pub(crate) struct Behaviour {
    pub dog: Toggle<libp2p_dog::Behaviour>,
    pub gossipsub: Toggle<gossipsub::Behaviour>,
}

impl Behaviour {
    pub(crate) fn new(config: &Config, key: &Keypair, registry: &mut Registry) -> Self {
        let dog = if let Protocol::Dog = config.benchmark.protocol {
            Toggle::from(Some(
                libp2p_dog::Behaviour::new_with_metrics(
                    libp2p_dog::TransactionAuthenticity::Signed(key.clone()),
                    libp2p_dog::ConfigBuilder::default()
                        .target_redundancy(config.benchmark.redundancy)
                        .redundancy_delta_percent(config.benchmark.redundancy_delta)
                        .redundancy_interval(Duration::from_millis(
                            config.benchmark.redundancy_interval_in_ms,
                        ))
                        .max_transmit_size(MAX_TRANSMIT_SIZE)
                        .connection_handler_publish_duration(Duration::from_secs(10))
                        .connection_handler_forward_duration(Duration::from_secs(10))
                        .build()
                        .expect("Failed to build dog config"),
                    registry,
                )
                .expect("Failed to create dog behaviour"),
            ))
        } else {
            Toggle::from(None)
        };

        let gossipsub = if let Protocol::Gossipsub = config.benchmark.protocol {
            Toggle::from({
                let mut behaviour = gossipsub::Behaviour::new_with_metrics(
                    gossipsub::MessageAuthenticity::Signed(key.clone()),
                    gossipsub::ConfigBuilder::default()
                        .max_transmit_size(MAX_TRANSMIT_SIZE)
                        .publish_queue_duration(Duration::from_secs(10))
                        .forward_queue_duration(Duration::from_secs(10))
                        .build()
                        .expect("Failed to build gossipsub config"),
                    registry,
                    Default::default(),
                )
                .expect("Failed to create gossipsub behaviour");
                let topic = IdentTopic::new(GOSSIPSUB_TOPIC_STR);
                behaviour
                    .subscribe(&topic)
                    .expect("Failed to subscribe to gossipsub topic");
                Some(behaviour)
            })
        } else {
            Toggle::from(None)
        };

        Self { dog, gossipsub }
    }
}
