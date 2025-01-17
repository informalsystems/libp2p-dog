#[cfg(feature = "debug")]
use libp2p::{
    gossipsub,
    swarm::{self, SwarmEvent},
};

#[cfg(feature = "debug")]
use crate::{
    behaviour::{self, NetworkEvent},
    config::Config,
};

#[cfg(feature = "debug")]
async fn handle_dog_event(
    event: libp2p_dog::Event,
    _swarm: &mut swarm::Swarm<behaviour::Behaviour>,
    _config: &Config,
) {
    tracing::info!("Dog event: {:?}", event);
}

#[cfg(feature = "debug")]
async fn handle_gossipsub_event(
    event: gossipsub::Event,
    _swarm: &mut swarm::Swarm<behaviour::Behaviour>,
    _config: &Config,
) {
    tracing::info!("Gossipsub event: {:?}", event);
}

#[cfg(feature = "debug")]
async fn handle_swarm_specific_event(
    event: SwarmEvent<behaviour::NetworkEvent>,
    _swarm: &mut swarm::Swarm<behaviour::Behaviour>,
    _config: &Config,
) {
    tracing::info!("Swarm event: {:?}", event);
}

#[cfg(feature = "debug")]
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
