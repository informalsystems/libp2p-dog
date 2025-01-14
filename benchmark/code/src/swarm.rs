use std::time::Duration;

use libp2p::{noise, tcp, yamux, Swarm, SwarmBuilder};
use prometheus_client::registry::Registry;

use crate::{behaviour, config::Config};

pub(crate) fn new_swarm(config: &Config, registry: &mut Registry) -> Swarm<behaviour::Behaviour> {
    SwarmBuilder::with_new_identity()
        .with_tokio()
        .with_tcp(
            tcp::Config::default().nodelay(true),
            noise::Config::new,
            yamux::Config::default,
        )
        .unwrap()
        .with_behaviour(|key| behaviour::Behaviour::new(config, key, registry))
        .unwrap()
        .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(u64::MAX)))
        .build()
}
