use libp2p::{
    gossipsub,
    swarm::{self, SwarmEvent},
};

use crate::{
    behaviour::{self, NetworkEvent},
    config::Config,
    metrics::Metrics,
};

async fn handle_dog_event(
    event: libp2p_dog::Event,
    _swarm: &mut swarm::Swarm<behaviour::Behaviour>,
    _config: &Config,
    metrics: &mut Metrics,
) {
    match event {
        libp2p_dog::Event::Transaction { transaction_id, .. } => {
            metrics.add_delivered(
                transaction_id.0,
                std::time::UNIX_EPOCH.elapsed().unwrap().as_millis() as u64,
            );
        }
        _ => {}
    }
}

async fn handle_gossipsub_event(
    event: gossipsub::Event,
    _swarm: &mut swarm::Swarm<behaviour::Behaviour>,
    _config: &Config,
    metrics: &mut Metrics,
) {
    match event {
        gossipsub::Event::Message { message_id, .. } => {
            metrics.add_delivered(
                message_id.0,
                std::time::UNIX_EPOCH.elapsed().unwrap().as_millis() as u64,
            );
        }
        _ => {}
    }
}

pub(crate) async fn handle_swarm_event(
    event: SwarmEvent<behaviour::NetworkEvent>,
    swarm: &mut swarm::Swarm<behaviour::Behaviour>,
    config: &Config,
    metrics: &mut Metrics,
) {
    match event {
        SwarmEvent::Behaviour(NetworkEvent::Dog(event)) => {
            handle_dog_event(event, swarm, config, metrics).await;
        }
        SwarmEvent::Behaviour(NetworkEvent::Gossipsub(event)) => {
            handle_gossipsub_event(event, swarm, config, metrics).await;
        }
        _ => {}
    }
}
