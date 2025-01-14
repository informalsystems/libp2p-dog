use std::{
    error::Error,
    fs,
    io::{Seek, Write},
    time::Duration,
    u64,
};

use behaviour::GOSSIPSUB_TOPIC_STR;
use libp2p::{futures::StreamExt, gossipsub::IdentTopic, swarm::dial_opts::DialOpts};
use prometheus_client::{encoding::text::encode, registry::Registry};
use tokio::{select, time};

mod args;
mod behaviour;
mod config;
mod handler;
mod logging;
mod swarm;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    logging::init();

    let args = args::Args::new();

    let config = config::Config::new(&args);

    let mut registry = match config.benchmark.registry_prefix.clone() {
        ref prefix if prefix.is_empty() => Registry::default(),
        prefix => Registry::with_prefix(prefix),
    };

    let file = fs::File::create(format!("{}/{}.json", args.dir, config.node.id))?;
    let mut writer = std::io::BufWriter::new(&file);
    writer.write_all(b"{\"metrics\":[")?;

    let mut swarm = swarm::new_swarm(&config, &mut registry);

    swarm.listen_on(config.node.addr.clone())?;

    for peer_addr in &config.node.peers {
        swarm.dial(
            DialOpts::unknown_peer_id()
                .address(peer_addr.clone())
                .allocate_new_port()
                .build(),
        )?;
    }

    let transaction_interval = Duration::from_millis(1000 / config.benchmark.tps);
    let transaction_timer = time::sleep(Duration::from_secs(u64::MAX)); // Wait for start timestamp
    tokio::pin!(transaction_timer);

    let start_instant =
        time::Instant::now() + Duration::from_secs(config.benchmark.start_timestamp_in_sec);

    let stop_instant = time::Instant::now()
        + Duration::from_secs(
            config.benchmark.start_timestamp_in_sec + config.benchmark.duration_in_sec,
        );

    let dump_interval = Duration::from_millis(config.benchmark.dump_interval_in_ms);
    let dump_timer = time::sleep(Duration::from_secs(u64::MAX)); // Wait for start timestamp
    tokio::pin!(dump_timer);

    let gossipsub_topic = IdentTopic::new(GOSSIPSUB_TOPIC_STR);

    loop {
        select! {
            event = swarm.select_next_some() => {
                handler::handle_swarm_event(event, &mut swarm, &config).await;
            }

            _ = time::sleep_until(start_instant), if start_instant > time::Instant::now() => {
                tracing::info!("Benchmark started");
                transaction_timer.as_mut().reset(time::Instant::now() + transaction_interval);
                dump_timer.as_mut().reset(time::Instant::now() + dump_interval);
            }

            _ = &mut transaction_timer => {
                tracing::info!("Sending a transaction");
                match config.benchmark.protocol {
                    config::Protocol::Dog => {
                        match swarm.behaviour_mut()
                        .dog
                        .as_mut()
                        .expect("Dog behaviour should be enabled")
                        .publish(vec![0 as u8; config.benchmark.tx_size_in_bytes] as Vec<u8>) {
                            Ok(tx_id) => {
                                tracing::info!("Transaction sent with id {}", tx_id);
                            }
                            Err(e) => {
                                tracing::error!("Failed to send transaction: {:?}", e);
                            }
                        }
                    }
                    config::Protocol::Gossipsub => {
                        match swarm.behaviour_mut()
                        .gossipsub
                        .as_mut()
                        .expect("Gossipsub behaviour should be enabled")
                        .publish(gossipsub_topic.clone(), vec![0 as u8; config.benchmark.tx_size_in_bytes] as Vec<u8>) {
                            Ok(msg_id) => {
                                tracing::info!("Message sent with id {}", msg_id);
                            }
                            Err(e) => {
                                tracing::error!("Failed to send message: {:?}", e);
                            }
                        }
                    }
                }

                transaction_timer.as_mut().reset(time::Instant::now() + transaction_interval);
            }

            _ = &mut dump_timer => {
                tracing::info!("Dumping metrics");

                if let Err(e) = dump_metrics(&mut writer, &registry) {
                    tracing::error!("Failed to dump metrics: {:?}", e);
                }

                dump_timer.as_mut().reset(time::Instant::now() + Duration::from_millis(config.benchmark.dump_interval_in_ms));
            }

            _ = time::sleep_until(stop_instant) => {
                break;
            }
        }
    }

    tracing::info!("Benchmark finished");

    writer.seek(std::io::SeekFrom::End(-1))?; // Remove trailing comma
    writer.write_all(b"]}")?;

    Ok(())
}

fn dump_metrics(mut writer: impl Write, registry: &Registry) -> Result<(), Box<dyn Error>> {
    let mut output = String::new();
    match encode(&mut output, &registry) {
        Ok(()) => {
            let mut metrics = serde_json::Map::new();

            metrics.insert(
                "timestamp".to_string(),
                serde_json::Value::Number(serde_json::Number::from(
                    std::time::UNIX_EPOCH.elapsed().unwrap().as_millis() as u64,
                )),
            );

            for line in output.lines() {
                if !line.starts_with('#') {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() == 2 {
                        metrics.insert(
                            parts[0].to_string(),
                            parts[1].parse::<serde_json::Value>().unwrap(),
                        );
                    }
                }
            }

            let metrics = serde_json::Value::Object(metrics);
            serde_json::to_writer(&mut writer, &metrics)?;
            writer.write_all(b",")?;
        }
        Err(e) => Err(e)?,
    }

    Ok(())
}
