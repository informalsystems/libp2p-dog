use libp2p::{
    gossipsub,
    swarm::{self, SwarmEvent},
};

use crate::{
    behaviour::{self, NetworkEvent},
    config::Config,
};

async fn handle_dog_event(
    event: libp2p_dog::Event,
    _swarm: &mut swarm::Swarm<behaviour::Behaviour>,
    _config: &Config,
) {
    match event {
        libp2p_dog::Event::Transaction { transaction, .. } => {
            tracing::info!(
                "Received transaction: {}",
                match String::from_utf8(transaction.data.clone()) {
                    Ok(data) => data,
                    Err(_) => "Invalid UTF-8".to_string(),
                }
            );
        }
        libp2p_dog::Event::RoutingUpdated { disabled_routes } => {
            tracing::info!("Updated routing table: {:?}", disabled_routes);
        }
    }
}

async fn handle_gossipsub_event(
    event: gossipsub::Event,
    _swarm: &mut swarm::Swarm<behaviour::Behaviour>,
    _config: &Config,
) {
    tracing::info!("Gossipsub event: {:?}", event);
}

async fn handle_swarm_specific_event(
    event: SwarmEvent<behaviour::NetworkEvent>,
    _swarm: &mut swarm::Swarm<behaviour::Behaviour>,
    _config: &Config,
) {
    tracing::info!("Swarm event: {:?}", event);
}

pub(crate) async fn handle_swarm_event(
    event: SwarmEvent<behaviour::NetworkEvent>,
    swarm: &mut swarm::Swarm<behaviour::Behaviour>,
    config: &Config,
) {
    match event {
        SwarmEvent::Behaviour(NetworkEvent::Dog(event)) => {
            handle_dog_event(event, swarm, config).await;
        }
        SwarmEvent::Behaviour(NetworkEvent::Gossipsub(event)) => {
            handle_gossipsub_event(event, swarm, config).await;
        }
        _ => {
            handle_swarm_specific_event(event, swarm, config).await;
        }
    }
}
