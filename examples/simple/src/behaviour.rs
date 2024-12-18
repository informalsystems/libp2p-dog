use std::time::Duration;

use libp2p::{identify, identity::Keypair, swarm::NetworkBehaviour, PeerId};

use crate::config::Config;

fn identify_config(key: &Keypair) -> identify::Config {
    identify::Config::new("/malachite/id/1.0.0".to_string(), key.public())
        .with_interval(Duration::from_nanos(u64::MAX))
}

#[derive(Debug)]
pub(crate) enum NetworkEvent {
    Dog(libp2p_dog::DogEvent),
    Identify(identify::Event),
}

impl From<libp2p_dog::DogEvent> for NetworkEvent {
    fn from(event: libp2p_dog::DogEvent) -> Self {
        Self::Dog(event)
    }
}

impl From<identify::Event> for NetworkEvent {
    fn from(event: identify::Event) -> Self {
        Self::Identify(event)
    }
}

#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "NetworkEvent")]
pub(crate) struct MyBehaviour {
    pub dog: libp2p_dog::Behaviour,
    pub identify: identify::Behaviour,
}

impl MyBehaviour {
    pub(crate) fn new(_config: &Config, key: &Keypair) -> Self {
        let dog = libp2p_dog::Behaviour::new(PeerId::from_public_key(&key.public()));

        let identify = identify::Behaviour::new(identify_config(key));

        Self { dog, identify }
    }
}
