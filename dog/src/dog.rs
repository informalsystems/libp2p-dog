use std::{collections::HashMap, fmt::Display};

use libp2p::PeerId;
use rand::seq::IteratorRandom;

use crate::Config;

#[derive(Debug, Clone, PartialEq, Copy)]
pub struct Route {
    source: PeerId,
    target: PeerId,
}

impl Route {
    pub fn new(source: PeerId, target: PeerId) -> Self {
        Route { source, target }
    }

    pub fn source(&self) -> &PeerId {
        &self.source
    }

    pub fn target(&self) -> &PeerId {
        &self.target
    }
}

impl Display for Route {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Route({} -> {})", self.source, self.target)
    }
}

pub(crate) struct Router {
    disabled_routes: Vec<Route>,
    have_tx_sent_per_peer: HashMap<PeerId, usize>,
}

impl Router {
    pub(crate) fn new() -> Self {
        Router {
            disabled_routes: Vec::new(),
            have_tx_sent_per_peer: HashMap::new(),
        }
    }

    pub(crate) fn get_disabled_routes(&self) -> Vec<Route> {
        self.disabled_routes.clone()
    }

    pub(crate) fn disable_route(&mut self, source: PeerId, target: PeerId) {
        self.disabled_routes.push(Route::new(source, target));
    }

    pub(crate) fn enable_random_route_to_peer(&mut self, peer: PeerId) -> Option<Route> {
        if let Some((index, _)) = self
            .disabled_routes
            .iter()
            .enumerate()
            .filter(|(_, route)| route.target() == &peer)
            .choose(&mut rand::thread_rng())
        {
            return Some(self.disabled_routes.remove(index));
        }
        None
    }

    pub(crate) fn reset_routes_with_peer(&mut self, peer: PeerId) -> Vec<Route> {
        let mut removed_routes = Vec::new();
        self.disabled_routes.retain(|route| {
            if route.source() == &peer || route.target() == &peer {
                removed_routes.push(route.clone());
                false
            } else {
                true
            }
        });
        removed_routes
    }

    pub(crate) fn filter_valid_routes(&self, source: PeerId, targets: Vec<PeerId>) -> Vec<PeerId> {
        targets
            .into_iter()
            .filter(|target| {
                !self
                    .disabled_routes
                    .iter()
                    .any(|route| route.source() == &source && route.target() == target)
            })
            .collect()
    }

    pub(crate) fn register_have_tx_sent(&mut self, peer: PeerId) {
        let counter = self.have_tx_sent_per_peer.entry(peer).or_insert(0);
        *counter += 1;
    }

    // Returns a random peer to which we have sent a have_tx message.
    pub(crate) fn get_random_have_tx_sent_peer(&self) -> Option<PeerId> {
        self.have_tx_sent_per_peer
            .iter()
            .filter_map(|(peer, count)| if *count > 0 { Some(peer.clone()) } else { None })
            .choose(&mut rand::thread_rng())
    }

    pub(crate) fn remove_have_tx_sent(&mut self, peer: &PeerId) {
        if let Some(counter) = self.have_tx_sent_per_peer.get_mut(peer) {
            if *counter > 0 {
                *counter -= 1;
            }
        }
    }
}

pub(crate) struct Controller {
    lower_bound: f64,
    upper_bound: f64,
    first_time_txs_count: usize,
    duplicate_txs_count: usize,
    is_have_tx_blocked: bool,
}

impl Controller {
    pub(crate) fn new(config: &Config) -> Self {
        let target_redundancy = config.target_redundancy();
        let delta = target_redundancy * (config.redundancy_delta_percent() as f64 / 100.0);

        Controller {
            lower_bound: target_redundancy - delta,
            upper_bound: target_redundancy + delta,
            first_time_txs_count: 0,
            duplicate_txs_count: 0,
            is_have_tx_blocked: false,
        }
    }

    pub(crate) fn incr_first_time_txs_count(&mut self) {
        self.first_time_txs_count += 1;
    }

    pub(crate) fn incr_duplicate_txs_count(&mut self) {
        self.duplicate_txs_count += 1;
    }

    pub(crate) fn is_have_tx_blocked(&self) -> bool {
        self.is_have_tx_blocked
    }

    pub(crate) fn block_have_tx(&mut self) {
        self.is_have_tx_blocked = true;
    }

    fn redundancy(&self) -> f64 {
        if self.first_time_txs_count == 0 {
            self.upper_bound
        } else {
            self.duplicate_txs_count as f64 / self.first_time_txs_count as f64
        }
    }

    /// Evaluate the redundancy, unblock have_tx if necessary, and return whether
    /// we should send a reset route message.
    pub(crate) fn evaluate(&mut self) -> (f64, bool) {
        // Do not evaluate redundancy if no transactions have been received from the
        // last evaluation.
        if self.first_time_txs_count + self.duplicate_txs_count == 0 {
            return (self.upper_bound, false);
        }
        let redundancy = self.redundancy();
        // We do not have enough redundancy, so we request a reset route.
        if redundancy < self.lower_bound {
            return (redundancy, true);
        }
        // We have too much redundancy, so we unblock have_tx.
        if redundancy > self.upper_bound {
            self.is_have_tx_blocked = false;
        }
        (redundancy, false)
    }

    pub(crate) fn reset_counters(&mut self) {
        self.first_time_txs_count = 0;
        self.duplicate_txs_count = 0;
    }
}
