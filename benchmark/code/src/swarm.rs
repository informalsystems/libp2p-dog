use std::time::Duration;

use libp2p::{noise, tcp, yamux, Swarm, SwarmBuilder};

use crate::config::Config;

pub(crate) fn new_swarm(config: &Config) -> Swarm<libp2p_dog::Behaviour> {
    SwarmBuilder::with_new_identity()
        .with_tokio()
        .with_tcp(
            tcp::Config::default().nodelay(true),
            noise::Config::new,
            yamux::Config::default,
        )
        .unwrap()
        .with_behaviour(|key| {
            libp2p_dog::Behaviour::new(
                libp2p_dog::TransactionAuthenticity::Signed(key.clone()),
                libp2p_dog::Config::default(),
            )
            .expect("Failed to create dog behaviour")
        })
        .unwrap()
        .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(u64::MAX)))
        .build()
}
