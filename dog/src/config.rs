use libp2p::PeerId;

pub struct DogConfig {
    pub local_peer_id: PeerId,
}

impl DogConfig {
    pub fn new(local_peer_id: PeerId) -> Self {
        Self { local_peer_id }
    }
}
