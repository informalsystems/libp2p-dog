use libp2p::{
    core::ConnectedPoint,
    identify,
    swarm::{self, SwarmEvent},
};
use tracing::{info, warn};

use crate::{
    behaviour::{MyBehaviour, NetworkEvent},
    config::Config,
    state::State,
};

async fn handle_identify_event(
    event: identify::Event,
    _swarm: &mut swarm::Swarm<MyBehaviour>,
    _config: &Config,
    state: &mut State,
) {
    match event {
        identify::Event::Received { peer_id, info, .. } => {
            info!("Received identify info from {}", peer_id);

            state.peers.insert(peer_id, info);
        }
        identify::Event::Sent { peer_id, .. } => {
            info!("Sent identify info to {}", peer_id);
        }
        _ => {}
    }
}

async fn handle_dog_event(
    event: libp2p_dog::Event,
    _swarm: &mut swarm::Swarm<MyBehaviour>,
    _config: &Config,
    state: &mut State,
) {
    match event {
        libp2p_dog::Event::Transaction { transaction, .. } => {
            info!(
                "Received transaction: {}",
                match String::from_utf8(transaction.data.clone()) {
                    Ok(data) => data,
                    Err(_) => "Invalid UTF-8".to_string(),
                }
            );

            state.transactions_received.push(transaction);
        }
    }
}

async fn handle_swarm_specific_event(
    event: SwarmEvent<NetworkEvent>,
    _swarm: &mut swarm::Swarm<MyBehaviour>,
    _config: &Config,
    _state: &mut State,
) {
    match event {
        SwarmEvent::NewListenAddr { address, .. } => {
            info!("Listening on {}", address);
        }
        SwarmEvent::ConnectionEstablished {
            peer_id,
            endpoint,
            connection_id,
            ..
        } => match endpoint {
            ConnectedPoint::Dialer { .. } => {
                info!(
                    "Connected to {}, with connection id {}",
                    peer_id, connection_id
                );
            }
            ConnectedPoint::Listener { .. } => {
                info!("New connection from {}", peer_id);
            }
        },
        SwarmEvent::OutgoingConnectionError { error, .. } => {
            warn!("Failed to establish connection: {}", error);
        }
        SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
            info!("Connection to {} closed: {:?}", peer_id, cause);
        }
        _ => {}
    }
}

pub(crate) async fn handle_swarm_event(
    event: SwarmEvent<NetworkEvent>,
    swarm: &mut swarm::Swarm<MyBehaviour>,
    config: &Config,
    state: &mut State,
) {
    match event {
        SwarmEvent::Behaviour(NetworkEvent::Identify(event)) => {
            handle_identify_event(event, swarm, config, state).await;
        }
        SwarmEvent::Behaviour(NetworkEvent::Dog(event)) => {
            handle_dog_event(event, swarm, config, state).await;
        }
        _ => {
            // Swarm specific events
            handle_swarm_specific_event(event, swarm, config, state).await;
        }
    }
}
