use std::error::Error;

use libp2p::swarm::dial_opts::DialOpts;

mod args;
mod config;
mod logging;
mod swarm;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    logging::init();

    let args = args::Args::new();

    let config = config::Config::new(&args);

    let mut swarm = swarm::new_swarm(&config);

    swarm.listen_on(config.addr)?;

    for peer_addr in &config.peers {
        swarm.dial(
            DialOpts::unknown_peer_id()
                .address(peer_addr.clone())
                .allocate_new_port()
                .build(),
        )?;
    }

    loop {}
}
