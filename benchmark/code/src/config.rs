use std::fs;

use libp2p::Multiaddr;
use serde::Deserialize;

use crate::args::Args;

pub(crate) enum Protocol {
    Dog,
    Gossipsub,
}

impl<'de> Deserialize<'de> for Protocol {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "dog" => Ok(Protocol::Dog),
            "gossipsub" => Ok(Protocol::Gossipsub),
            _ => Err(serde::de::Error::custom("Invalid protocol")),
        }
    }
}

#[derive(Deserialize)]
pub(crate) struct Config {
    pub node: Node,
    pub benchmark: Benchmark,
}

#[derive(Deserialize)]
pub(crate) struct Node {
    pub id: String,
    pub addr: Multiaddr,
    pub peers: Vec<Multiaddr>,
}

#[derive(Deserialize)]
pub(crate) struct Benchmark {
    pub protocol: Protocol,
    pub duration_in_sec: u64,
    pub tps: u64,
    pub tx_size_in_bytes: usize,
    pub dump_interval_in_ms: u64,
    pub registry_prefix: String,
}

impl Config {
    pub(crate) fn new(args: &Args) -> Self {
        let config = fs::read_to_string(&args.config).expect("Failed to read config file");
        toml::from_str(&config).expect("Failed to parse config file")
    }
}
