use libp2p::{identity::Keypair, swarm::NetworkBehaviour};
use prometheus_client::registry::Registry;

use crate::config::Config;

#[derive(Debug)]
pub(crate) enum NetworkEvent {
    Dog(libp2p_dog::Event),
}

impl From<libp2p_dog::Event> for NetworkEvent {
    fn from(event: libp2p_dog::Event) -> Self {
        Self::Dog(event)
    }
}

#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "NetworkEvent")]
pub(crate) struct MyBehaviour {
    pub dog: libp2p_dog::Behaviour,
}

impl MyBehaviour {
    pub(crate) fn new(_config: &Config, key: &Keypair, registry: &mut Registry) -> Self {
        let dog = libp2p_dog::Behaviour::new_with_metrics(
            libp2p_dog::TransactionAuthenticity::Signed(key.clone()),
            libp2p_dog::Config::default(),
            registry,
            // libp2p_dog::TransactionAuthenticity::Author(key.public().to_peer_id()),
            // libp2p_dog::ConfigBuilder::default()
            //     .validation_mode(libp2p_dog::ValidationMode::None)
            //     .build()
            //     .expect("Failed to create dog behaviour"),
        )
        .expect("Failed to create dog behaviour");

        Self { dog }
    }
}
