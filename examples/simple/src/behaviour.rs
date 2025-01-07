use libp2p::{identity::Keypair, swarm::NetworkBehaviour, PeerId};

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
    pub(crate) fn new(_config: &Config, key: &Keypair) -> Self {
        let dog = libp2p_dog::Behaviour::new(
            libp2p_dog::TransactionAuthenticity::Author(PeerId::from_public_key(&key.public())),
            libp2p_dog::Config::default(),
        )
        .expect("Failed to create dog behaviour");

        Self { dog }
    }
}
