use std::str::FromStr;

use libp2p::Multiaddr;

use crate::args::Args;

#[derive(Debug)]
pub(crate) struct Config {
    pub addr: Multiaddr,
    pub peers: Vec<Multiaddr>,
}

impl Config {
    pub(crate) fn new(args: &Args) -> Self {
        let addr = Multiaddr::from_str(&format!("/ip4/127.0.0.1/tcp/{}", args.port))
            .expect("Failed to parse address");

        let peers = args
            .peers
            .iter()
            .map(|peer| Multiaddr::from_str(peer).expect("Failed to parse peer"))
            .collect();

        Self { addr, peers }
    }
}
