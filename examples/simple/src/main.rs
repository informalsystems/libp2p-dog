use std::{error::Error, time::Duration};

use libp2p::{futures::StreamExt, swarm::dial_opts::DialOpts};
use prometheus_client::registry::Registry;
use tokio::{select, time};
use tracing::{debug, info};

mod args;
mod behaviour;
mod config;
mod handler;
mod logging;
mod state;
mod swarm;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    logging::init();

    let args = args::Args::new();
    debug!("Args: {:?}", args);

    let config = config::Config::new(&args);

    let mut registry = Registry::with_prefix("dog");

    let mut swarm = swarm::new_swarm(&config, &mut registry);

    swarm.listen_on(format!("/ip4/127.0.0.1/tcp/{}", args.port).parse()?)?;

    let mut state = state::State::new(&config);

    if let Some(node) = &config.node {
        swarm.dial(
            DialOpts::unknown_peer_id()
                .address(node.clone())
                .allocate_new_port()
                .build(),
        )?;
    }

    let mut i = 0;
    let local_peer_id = swarm.local_peer_id().clone();

    let sleep = time::sleep(Duration::from_secs(5));
    tokio::pin!(sleep);

    let stop = time::sleep(Duration::from_secs(20));
    tokio::pin!(stop);

    loop {
        select! {
            event = swarm.select_next_some() => {
                handler::handle_swarm_event(event, &mut swarm, &config, &mut state).await;
            }

            _ = &mut sleep => {
                info!("Sending a transaction");

                match swarm.behaviour_mut().dog.publish(format!("transaction #{i} from {}", local_peer_id)) {
                    Ok(tx_id) => {
                        info!("Transaction sent with id {}", tx_id);
                    }
                    Err(e) => {
                        info!("Failed to send transaction: {:?}", e);
                    }
                }

                i += 1;

                sleep.as_mut().reset(time::Instant::now() + Duration::from_secs(5));
            }

            _ = &mut stop => {
                info!("Stopping the node");
                break;
            }
        }
    }

    Ok(())
}
