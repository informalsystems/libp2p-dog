use std::{str::FromStr, time::Duration};

use libp2p::{
    futures::StreamExt, identity::Keypair, swarm::dial_opts::DialOpts, Multiaddr, PeerId,
    SwarmBuilder,
};
use libp2p_dog::{IdentityTransform, Route};
use rand::Rng;
use tokio::{sync::mpsc, task::JoinHandle, time::sleep};

pub struct Test<const N: usize> {
    nodes: [TestNode; N],
}

impl<const N: usize> Test<N> {
    pub fn new_with_unique_config(
        config: libp2p_dog::Config,
        bootstrap_sets: [Vec<usize>; N],
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let base_port = rand::thread_rng().gen_range(21000..50000);

        let nodes = (0..N)
            .map(|i| {
                let addr =
                    Multiaddr::from_str(&format!("/ip4/127.0.0.1/tcp/{}", base_port + i)).unwrap();
                let bootstrap_set = bootstrap_sets[i]
                    .iter()
                    .map(|j| {
                        Multiaddr::from_str(&format!("/ip4/127.0.0.1/tcp/{}", base_port + *j))
                            .unwrap()
                    })
                    .collect();

                TestNode::new(addr, bootstrap_set, config.clone()).unwrap()
            })
            .collect::<Vec<_>>()
            .try_into()
            .map_err(|_| "Failed to convert Vec to array")?;

        Ok(Self { nodes })
    }

    pub async fn spawn_all(&mut self) -> Vec<JoinHandle<()>> {
        let join_handlers = self.nodes.iter_mut().map(|node| node.spawn()).collect();
        // Wait for the swarms to initialize and dial each other
        sleep(Duration::from_secs(2)).await;
        join_handlers
    }

    pub fn peer_ids(&self) -> [PeerId; N] {
        self.nodes
            .iter()
            .map(|node| node.peer_id())
            .collect::<Vec<_>>()
            .try_into()
            .unwrap()
    }

    pub fn publish_on_node(&self, node: usize, data: Vec<u8>) {
        assert!(node < N);
        self.nodes[node].publish(data);
    }

    pub fn collect_events(&mut self) -> [(Vec<libp2p_dog::Transaction>, Vec<Vec<Route>>); N] {
        self.nodes
            .iter_mut()
            .map(|node| node.collect_events())
            .collect::<Vec<_>>()
            .try_into()
            .map_err(|_| "Failed to convert Vec to array")
            .unwrap()
    }
}

pub enum Event {
    Dog(libp2p_dog::Event),
    Error(String),
}

pub struct TestNode {
    keypair: Keypair,
    peer_id: PeerId,
    addr: Multiaddr,
    bootstrap_set: Vec<Multiaddr>,
    config: libp2p_dog::Config,
    tx_event: mpsc::UnboundedSender<Event>,
    rx_event: mpsc::UnboundedReceiver<Event>,
    tx_publish: Option<mpsc::UnboundedSender<Vec<u8>>>,
}

impl TestNode {
    pub fn new(
        addr: Multiaddr,
        bootstrap_set: Vec<Multiaddr>,
        config: libp2p_dog::Config,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let keypair = Keypair::generate_ed25519();
        let peer_id = PeerId::from_public_key(&keypair.public());

        let (tx_event, rx_event) = mpsc::unbounded_channel();

        Ok(Self {
            keypair,
            peer_id,
            addr,
            bootstrap_set,
            config,
            tx_event,
            rx_event,
            tx_publish: None,
        })
    }

    pub fn peer_id(&self) -> PeerId {
        self.peer_id.clone()
    }

    pub fn publish(&self, data: Vec<u8>) {
        if let Some(tx) = &self.tx_publish {
            tx.send(data).unwrap_or_else(|e| {
                println!("Failed to send publish message: {}", e);
            });
        }
    }

    pub fn spawn(&mut self) -> JoinHandle<()> {
        let keypair = self.keypair.clone();
        let addr = self.addr.clone();
        let bootstrap_set = self.bootstrap_set.clone();
        let config = self.config.clone();
        let tx_event = self.tx_event.clone();
        let (tx_publish, mut rx_publish) = mpsc::unbounded_channel::<Vec<u8>>();

        self.tx_publish = Some(tx_publish);

        tokio::spawn(async move {
            let mut swarm = SwarmBuilder::with_existing_identity(keypair)
                .with_tokio()
                .with_tcp(
                    libp2p::tcp::Config::new().nodelay(true), // Disable Nagle's algorithm
                    libp2p::noise::Config::new,
                    libp2p::yamux::Config::default,
                )
                .unwrap()
                .with_behaviour(|key| {
                    libp2p_dog::Behaviour::<IdentityTransform>::new(
                        libp2p_dog::TransactionAuthenticity::Signed(key.clone()),
                        config,
                    )
                    .expect("Failed to create dog behaviour")
                })
                .unwrap()
                .with_swarm_config(|cfg| {
                    cfg.with_idle_connection_timeout(std::time::Duration::from_secs(u64::MAX))
                })
                .build();

            match swarm.listen_on(addr.clone()) {
                Ok(_) => {}
                Err(err) => {
                    tx_event
                        .send(Event::Error(format!(
                            "Failed to listen on address: {}",
                            err
                        )))
                        .unwrap_or_else(|e| {
                            println!("Failed to send error message: {}", e);
                        });
                    return;
                }
            }

            // Wait for other swarm to start before dialing
            sleep(Duration::from_secs(1)).await;

            for node in bootstrap_set {
                match swarm.dial(
                    DialOpts::unknown_peer_id()
                        .address(node)
                        .allocate_new_port()
                        .build(),
                ) {
                    Ok(_) => {}
                    Err(err) => {
                        tx_event
                            .send(Event::Error(format!("Failed to dial node: {}", err)))
                            .unwrap_or_else(|e| println!("Failed to send error message {}", e));
                        return;
                    }
                }
            }

            loop {
                tokio::select! {
                    event = swarm.select_next_some() => {
                        match event {
                            libp2p::swarm::SwarmEvent::Behaviour(ev) => {
                                tx_event.send(Event::Dog(ev)).unwrap_or_else(|e| {
                                    println!("Failed to send dog event: {}", e);
                                    return;
                                });
                            }
                            _ => {}
                        }
                    }

                    Some(data) = rx_publish.recv() => {
                        match swarm.behaviour_mut().publish(data) {
                            Ok(_) => {}
                            Err(err) => {
                                tx_event.send(Event::Error(format!("Failed to publish data: {}", err))).unwrap_or_else(|e| {
                                    println!("Failed to send error message: {}", e);
                                    return;
                                });
                            }
                        }
                    }
                }
            }
        })
    }

    pub fn collect_events(&mut self) -> (Vec<libp2p_dog::Transaction>, Vec<Vec<Route>>) {
        let mut txns = Vec::new();
        let mut routing_updates = Vec::new();

        while let Ok(event) = self.rx_event.try_recv() {
            match event {
                Event::Dog(libp2p_dog::Event::Transaction { transaction, .. }) => {
                    txns.push(transaction);
                }
                Event::Dog(libp2p_dog::Event::RoutingUpdated { disabled_routes }) => {
                    routing_updates.push(disabled_routes);
                }
                Event::Error(err) => {
                    panic!("Error: {}", err);
                }
            }
        }

        (txns, routing_updates)
    }
}
