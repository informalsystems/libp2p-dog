use libp2p::{
    gossipsub::{self, IdentTopic},
    identity::Keypair,
    swarm::{behaviour::toggle::Toggle, NetworkBehaviour},
};
use prometheus_client::registry::Registry;

use crate::config::{Config, Protocol};

pub(crate) const GOSSIPSUB_TOPIC_STR: &str = "benchmark";

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
                    libp2p_dog::Config::default(),
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
                    gossipsub::Config::default(),
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
