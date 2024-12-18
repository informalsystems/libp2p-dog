use std::str::FromStr;

use crate::args::Args;
use libp2p::Multiaddr;

#[derive(Debug)]
pub(crate) struct Config {
    pub node: Option<Multiaddr>,
}

impl Config {
    pub(crate) fn new(args: &Args) -> Self {
        let multiaddr = match Multiaddr::from_str(&args.node) {
            Ok(multiaddr) => Some(multiaddr),
            Err(_) => None,
        };

        Self { node: multiaddr }
    }
}
