use crate::config::Config;
use libp2p::{identify::Info, PeerId};
use libp2p_dog::DogTransaction;
use std::collections::HashMap;

#[derive(Debug)]
pub(crate) struct State {
    pub peers: HashMap<PeerId, Info>,
    pub transactions_received: Vec<DogTransaction>,
}

impl State {
    pub(crate) fn new(_config: &Config) -> Self {
        Self {
            peers: HashMap::new(),
            transactions_received: Vec::new(),
        }
    }
}
