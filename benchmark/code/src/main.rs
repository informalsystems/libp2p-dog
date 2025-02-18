use std::{
    error::Error,
    fs,
    io::{Seek, Write},
    time::Duration,
    u64,
};

use behaviour::GOSSIPSUB_TOPIC_STR;
use libp2p::{futures::StreamExt, gossipsub::IdentTopic, swarm::dial_opts::DialOpts};
use metrics::Metrics;
use prometheus_client::{encoding::text::encode, registry::Registry};
use tokio::{select, time};

mod args;
mod behaviour;
mod config;
mod handler;
mod logging;
mod metrics;
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

    let mut metrics = Metrics::new();

    let file = fs::File::create(format!("{}/{}.json", args.dir, config.node.id))?;
    let mut writer = std::io::BufWriter::new(&file);
    writer.write_all(b"{\"metrics\":[")?;

    let mut swarm = swarm::new_swarm(&config, &mut registry);

    swarm.listen_on(config.node.addr.clone())?;

    time::sleep(Duration::from_secs(10)).await; // Make sure all other nodes are ready

    for peer_addr in &config.node.peers {
        swarm.dial(
            DialOpts::unknown_peer_id()
                .address(peer_addr.clone())
                .allocate_new_port()
                .build(),
        )?;
    }

    let start_instant = time::Instant::now()
        + Duration::from_millis(
            args.start_timestamp
                .saturating_sub(std::time::UNIX_EPOCH.elapsed()?.as_millis() as u64),
        );
    let start_timer = time::sleep_until(start_instant);
    tokio::pin!(start_timer);
    let mut started = false;

    let transaction_interval = Duration::from_millis(1000 / config.benchmark.tps);
    let transaction_timer = time::sleep(Duration::from_secs(u64::MAX)); // Wait for start timestamp
    tokio::pin!(transaction_timer);
    let mut next_transaction_instant = time::Instant::now(); // Dummy value

    let dump_interval = Duration::from_millis(config.benchmark.dump_interval_in_ms);
    let dump_timer = time::sleep(Duration::from_secs(u64::MAX)); // Wait for start timestamp
    tokio::pin!(dump_timer);
    let mut next_dump_instant = time::Instant::now(); // Dummy value

    let stop_instant = start_instant
        + Duration::from_secs(config.benchmark.duration_in_sec)
        + Duration::from_secs(config.benchmark.stop_delay_in_sec);
    let stop_timer = time::sleep_until(stop_instant);
    tokio::pin!(stop_timer);

    let gossipsub_topic = IdentTopic::new(GOSSIPSUB_TOPIC_STR);

    let total_transactions = config.benchmark.tps * config.benchmark.duration_in_sec;
    let mut num_transactions: u64 = 0;

    tracing::info!(
        "Starting benchmark in {:?}",
        start_instant - time::Instant::now(),
    );

    loop {
        select! {
            event = swarm.select_next_some() => {
                handler::handle_swarm_event(event, &mut swarm, &config, &mut metrics).await;
            }

            _ = &mut start_timer, if !started => {
                tracing::info!("Benchmark started");
                started = true;
                next_transaction_instant = time::Instant::now();
                transaction_timer.as_mut().reset(next_transaction_instant);
                next_dump_instant = time::Instant::now();
                dump_timer.as_mut().reset(next_dump_instant);
            }

            _ = &mut transaction_timer, if num_transactions < total_transactions => {
                tracing::debug!("Sending a transaction");
                let timestamp = std::time::UNIX_EPOCH.elapsed().unwrap().as_millis() as u64;

                match config.benchmark.protocol {
                    config::Protocol::Dog => {
                        match swarm.behaviour_mut()
                        .dog
                        .as_mut()
                        .expect("Dog behaviour should be enabled")
                        .publish(vec![0 as u8; config.benchmark.tx_size_in_bytes] as Vec<u8>) {
                            Ok(tx_id) => {
                                tracing::debug!("Transaction sent with id {}", tx_id);

                                metrics.add_published(tx_id.0, timestamp);
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
                                tracing::debug!("Message sent with id {}", msg_id);

                                metrics.add_published(msg_id.0, timestamp);
                            }
                            Err(e) => {
                                tracing::error!("Failed to send message: {:?}", e);
                            }
                        }
                    }
                }

                next_transaction_instant += transaction_interval;
                transaction_timer.as_mut().reset(next_transaction_instant);
                num_transactions += 1;
            }

            _ = &mut dump_timer => {
                #[cfg(feature = "debug")]
                {
                    tracing::info!("Dumping metrics");
                }

                if let Err(e) = dump_metrics(&mut writer, &registry, &metrics) {
                    tracing::error!("Failed to dump metrics: {:?}", e);
                }

                next_dump_instant += dump_interval;
                dump_timer.as_mut().reset(next_dump_instant);
            }

            _ = &mut stop_timer => {
                break;
            }
        }
    }

    tracing::info!("Benchmark finished");

    writer.seek(std::io::SeekFrom::End(-1))?; // Remove trailing comma
    writer.write_all(b"]}")?;
    writer.flush()?;

    metrics.dump_metrics(
        format!("{}/published_{}.json", args.dir, config.node.id),
        format!("{}/delivered_{}.json", args.dir, config.node.id),
    )?;

    Ok(())
}

fn dump_metrics(
    mut writer: impl Write,
    registry: &Registry,
    metrics: &Metrics,
) -> Result<(), Box<dyn Error>> {
    let mut output = String::new();
    match encode(&mut output, &registry) {
        Ok(()) => {
            let mut metrics_map = serde_json::Map::new();

            metrics_map.insert(
                "timestamp".to_string(),
                serde_json::Value::Number(serde_json::Number::from(
                    std::time::UNIX_EPOCH.elapsed().unwrap().as_millis() as u64,
                )),
            );

            metrics_map.insert(
                "total_delivered".to_string(),
                serde_json::Value::Number(serde_json::Number::from(metrics.total_delivered())),
            );

            for line in output.lines() {
                if !line.starts_with('#') {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() == 2 {
                        metrics_map.insert(
                            parts[0].to_string(),
                            parts[1].parse::<serde_json::Value>().unwrap(),
                        );
                    }
                }
            }

            let metrics_obj = serde_json::Value::Object(metrics_map);
            serde_json::to_writer(&mut writer, &metrics_obj)?;
            writer.write_all(b",")?;
            writer.flush()?;
        }
        Err(e) => Err(e)?,
    }

    Ok(())
}
