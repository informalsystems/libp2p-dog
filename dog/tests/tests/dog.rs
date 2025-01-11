use std::time::Duration;

use libp2p_dog_tests::Test;
use rand::seq::SliceRandom;
use tokio::time::sleep;

// Testing the dog behaviour with two nodes sending transactions to each other
//     0 <---> 1
#[tokio::test]
pub async fn two_nodes_bidirectional() {
    let config = libp2p_dog::ConfigBuilder::default()
        // No redundancy to avoid the nodes sending reset route messages
        .target_redundancy(0.0)
        .redundancy_delta_percent(0)
        .build()
        .unwrap();

    let bootstrap_sets = [vec![1], vec![0]];

    let mut test = match Test::<2>::new_with_unique_config(config, bootstrap_sets, true) {
        Ok(test) => test,
        Err(e) => panic!("Failed to create test: {}", e),
    };

    test.spawn_all().await;

    for i in 0..10 {
        test.publish_on_node(0, format!("Hello #{} from node 1!", i).into_bytes());
        test.publish_on_node(1, format!("Hello #{} from node 2!", i).into_bytes());
    }

    sleep(Duration::from_secs(5)).await;

    let peer_ids = test.peer_ids();
    let events = test.collect_events();

    assert_eq!(peer_ids.len(), 2);
    assert_eq!(events.len(), 2);

    for (i, (transactions, routing_updates)) in events.iter().enumerate() {
        assert_eq!(transactions.len(), 10);
        let expected = (0..10)
            .map(|j| {
                libp2p_dog::Transaction {
                    from: peer_ids[1 - i],
                    seqno: 0, // ignored
                    data: format!("Hello #{} from node {}!", j, 2 - i).into_bytes(),
                }
            })
            .collect::<Vec<_>>();

        for (j, transaction) in transactions.iter().enumerate() {
            let expected_transaction = &expected[j];
            assert_eq!(transaction.from, expected_transaction.from);
            assert_eq!(transaction.data, expected_transaction.data);
        }

        assert_eq!(routing_updates.len(), 0);
    }
}

// Testing the dog behaviour with n nodes aligned in a chain sending transactions to each other
//     0 <---> 1 <---> 2 <---> ... <---> n-1
#[tokio::test]
pub async fn n_nodes_aligned() {
    let config = libp2p_dog::ConfigBuilder::default()
        // No redundancy to avoid the nodes sending reset route messages
        .target_redundancy(0.0)
        .redundancy_delta_percent(0)
        .build()
        .unwrap();

    const N: usize = 5;

    let bootstrap_sets: [Vec<usize>; N] = (0..N)
        .map(|i| if i == 0 { vec![] } else { vec![i - 1] })
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();

    let mut test = match Test::<N>::new_with_unique_config(config, bootstrap_sets, true) {
        Ok(test) => test,
        Err(e) => panic!("Failed to create test: {}", e),
    };

    test.spawn_all().await;

    for i in 0..10 {
        for j in 0..N {
            test.publish_on_node(j, format!("Hello #{} from node {}!", i, j).into_bytes());
        }
    }

    sleep(Duration::from_secs(5)).await;

    let peer_ids = test.peer_ids();
    let events = test.collect_events();

    assert_eq!(peer_ids.len(), N);
    assert_eq!(events.len(), N);

    for (i, (transactions, routing_updates)) in events.iter().enumerate() {
        assert_eq!(transactions.len(), (N - 1) * 10);
        let mut expected = (0..10)
            .map(|j| {
                (0..N)
                    .filter(|k| *k != i)
                    .map(|k| libp2p_dog::Transaction {
                        from: peer_ids[k],
                        seqno: 0, // ignored
                        data: format!("Hello #{} from node {}!", j, k).into_bytes(),
                    })
                    .collect::<Vec<_>>()
            })
            .flatten()
            .collect::<Vec<_>>();

        for transaction in transactions {
            let index = match expected.iter().position(|expected| {
                expected.from == transaction.from && expected.data == transaction.data
            }) {
                Some(index) => index,
                None => panic!("Unexpected transaction: {:?}", transaction),
            };
            expected.remove(index);
        }

        assert_eq!(routing_updates.len(), 0);
    }
}

// Testing that a node receiving the same transaction from different nodes will request eventually
// request all of them except one to stop sending it.
// We consider a random network of size N with target redundancy set to 0.0.
// Each node will publish transactions at constant intervals. We expect that after a certain amount
// of time, the routing status of the network will be stable. Moreover, we expect that, for each
// transaction, there is a single associated tree-like route from the source (root) to all other
// nodes (leaves).
#[tokio::test]
pub async fn random_network_no_redundancy() {
    let config = libp2p_dog::ConfigBuilder::default()
        // We force the nodes to remove any redundancy
        .target_redundancy(0.0)
        .redundancy_delta_percent(0)
        // Speed up have_tx unblocking
        .redundancy_interval(Duration::from_millis(10))
        // Disable signature to speed up the test
        .validation_mode(libp2p_dog::ValidationMode::None)
        .build()
        .unwrap();

    const N: usize = 10;

    let bootstrap_sets = Test::<N>::random_network();

    let mut test = match Test::<N>::new_with_unique_config(config, bootstrap_sets.clone(), false) {
        Ok(test) => test,
        Err(e) => panic!("Failed to create test: {}", e),
    };

    test.spawn_all().await;

    for i in 0..(N - 1) * (N - 2) {
        for j in 0..N {
            test.publish_on_node(j, format!("Hello #{} from node {}!", i, j).into_bytes());
        }
        sleep(Duration::from_millis(100)).await;
    }

    sleep(Duration::from_secs(5)).await;

    let peer_ids = test.peer_ids();
    let events = test.collect_events();

    assert_eq!(peer_ids.len(), N);
    assert_eq!(events.len(), N);

    for (i, (transactions, _)) in events.iter().enumerate() {
        assert_eq!(transactions.len(), (N - 1) * (N - 2) * (N - 1));
        let mut expected = (0..(N - 1) * (N - 2))
            .map(|j| {
                (0..N)
                    .filter(|k| *k != i)
                    .map(|k| libp2p_dog::Transaction {
                        from: peer_ids[k],
                        seqno: 0, // ignored
                        data: format!("Hello #{} from node {}!", j, k).into_bytes(),
                    })
                    .collect::<Vec<_>>()
            })
            .flatten()
            .collect::<Vec<_>>();

        for transaction in transactions {
            let index = match expected.iter().position(|expected| {
                expected.from == transaction.from && expected.data == transaction.data
            }) {
                Some(index) => index,
                None => panic!("Unexpected transaction: {:?}", transaction),
            };
            expected.remove(index);
        }
    }

    // Verify that no reset route messages have been sent
    for (_, routing_updates) in events.iter() {
        for (j, routes) in routing_updates.iter().enumerate() {
            if j == 0 {
                continue;
            }

            assert!(routes.len() > routing_updates[j - 1].len());
        }
    }

    // Build the directed graph of the network
    let mut base_adjency_list: Vec<Vec<usize>> = vec![Vec::new(); N];
    for i in 0..N {
        for j in bootstrap_sets[i].iter() {
            base_adjency_list[i].push(*j);
        }
    }

    let peer_id_to_index = |peer_id: &libp2p::PeerId| -> usize {
        peer_ids.iter().position(|id| id == peer_id).unwrap()
    };

    for i in 0..N {
        let mut i_adjency_list = base_adjency_list.clone();

        for (j, (_, routing_updates)) in events.iter().enumerate() {
            match routing_updates.last() {
                Some(routes) => {
                    for route in routes.iter().filter(|r| r.source() == &peer_ids[i]) {
                        i_adjency_list[j]
                            .retain(|target| *target != peer_id_to_index(route.target()));
                    }
                }
                None => {
                    continue;
                }
            };
        }

        let mut visited = vec![false; N];
        let mut stack = vec![(i, i)]; // (node, parent)
        while let Some((node, parent)) = stack.pop() {
            visited[node] = true;
            for neighbor in i_adjency_list[node].iter() {
                if *neighbor == parent {
                    // A -> B and B -> A is not considered as a cycle
                    continue;
                }
                if visited[*neighbor] {
                    panic!("Cycle detected between nodes {} and {}", node, *neighbor);
                }
                stack.push((*neighbor, node));
            }
        }

        for visited in visited.iter() {
            assert!(*visited);
        }
    }
}

// Testing that a node will request to reset a route previously blocked by a have_tx message if
// the redundancy is too low.
// We consider the following network:
//     A <---> Bi <---> C
// where A is the transaction sender, Bi (1 <= i <= N) are intermediary nodes and C is the node
// on which we want to test the behaviour.
// We define a redundancy R (0 <= R <= N - 1). First, A publishes a bunch of transactions. After
// some time, C should have sent N - R have_tx messages to nodes Bi (in order to reach the target
// redundancy). Then, A continues to publish transactions, but we kill M nodes Bi (0 <= M <= N - R,
// i.e. the ones that have not received a have_tx message from C). We expect C to reset some routes
// to recover the target redundancy.
// For a better test tracking, we define N >= (2 * R) + 1 to make sure node C will be able to
// reach the target redundancy.
#[tokio::test]
pub async fn simple_reset_route_scenario() {
    const R: usize = 3;
    const B: usize = 2 * R + 1; // Number of Bi nodes
    const N: usize = B + 2; // Total number of nodes

    let config_a_bi = libp2p_dog::ConfigBuilder::default()
        // We force the nodes to remove any redundancy
        .target_redundancy(0.0)
        .redundancy_delta_percent(0)
        // Speed up have_tx unblocking
        .redundancy_interval(Duration::from_secs(100))
        // Disable signature to speed up the test
        .validation_mode(libp2p_dog::ValidationMode::None)
        .build()
        .unwrap();
    let config_c = libp2p_dog::ConfigBuilder::default()
        // We force the nodes to remove any redundancy
        .target_redundancy(R as f64)
        .redundancy_delta_percent(0)
        // Speed up have_tx unblocking
        .redundancy_interval(Duration::from_millis(10))
        // Disable signature to speed up the test
        .validation_mode(libp2p_dog::ValidationMode::None)
        // For simplicity, node C acts as a client
        .forward_transactions(false)
        .build()
        .unwrap();

    let mut configs = Vec::with_capacity(N);
    for _ in 0..B + 1 {
        configs.push(config_a_bi.clone());
    }
    configs.push(config_c);

    let mut bootstrap_sets: Vec<Vec<usize>> = Vec::with_capacity(N);
    bootstrap_sets.push((1..B + 1).collect());
    for _ in 0..B {
        bootstrap_sets.push(vec![N - 1]);
    }
    bootstrap_sets.push(vec![]);

    let mut test = match Test::<N>::new_with_each_config(
        match configs.try_into() {
            Ok(configs_array) => configs_array,
            Err(_) => panic!("Failed to convert Vec to array"),
        },
        bootstrap_sets
            .try_into()
            .expect("Failed to convert Vec to array"),
        false,
    ) {
        Ok(test) => test,
        Err(e) => panic!("Failed to create test: {}", e),
    };

    test.spawn_all().await;

    for i in 0..B - R + 1 {
        test.publish_on_node(0, format!("Hello #{} from node A!", i).into_bytes());
        sleep(Duration::from_millis(100)).await;
    }

    sleep(Duration::from_secs(5)).await;

    let peer_ids = test.peer_ids();
    let events = test.collect_events();

    assert_eq!(peer_ids.len(), N);
    assert_eq!(events.len(), N);

    for (i, (transactions, _)) in events.iter().enumerate() {
        if i == 0 {
            // Node A
            continue;
        }

        assert_eq!(transactions.len(), B - R + 1);
        let mut expected = (0..B - R + 1)
            .map(|j| libp2p_dog::Transaction {
                from: peer_ids[0],
                seqno: 0, // ignored
                data: format!("Hello #{} from node A!", j).into_bytes(),
            })
            .collect::<Vec<_>>();

        for transaction in transactions {
            let index = match expected.iter().position(|expected| {
                expected.from == transaction.from && expected.data == transaction.data
            }) {
                Some(index) => index,
                None => panic!("Unexpected transaction: {:?}", transaction),
            };
            expected.remove(index);
        }
    }

    let mut exception = false;
    for (i, (_, routing_updates)) in events.iter().enumerate() {
        if i == 0 || i == N - 1 {
            assert!(routing_updates.is_empty());
            continue;
        }

        match routing_updates.len() {
            0 => {}
            1 => {
                assert_eq!(routing_updates[0].len(), 1);
            }
            2 => {
                if exception {
                    panic!("There can only be one exception");
                }
                exception = true;
                assert_eq!(routing_updates[0].len(), 1);
                assert_eq!(routing_updates[1].len(), 0);
            }
            n => panic!("Unexpected number of routing updates: {}", n),
        }
    }

    // Retrieve the Bi nodes that have not received a have_tx message from C
    let mut bi_nodes_to_kill = Vec::new();
    for (i, (_, routing_updates)) in events.iter().enumerate() {
        if !(1..B + 1).contains(&i) {
            continue;
        }

        match routing_updates.last() {
            Some(routes) => {
                if routes.is_empty() {
                    bi_nodes_to_kill.push(i);
                }
            }
            None => {
                bi_nodes_to_kill.push(i);
            }
        }
    }

    // Drop a random Bi node as we need to keep one route alive
    bi_nodes_to_kill.shuffle(&mut rand::thread_rng());
    bi_nodes_to_kill.pop();

    // Kill the Bi nodes that have not received a have_tx message from C
    for bi_node in bi_nodes_to_kill.iter() {
        test.kill_node(*bi_node).await;
    }

    for i in B - R - 1..B - 1 {
        test.publish_on_node(0, format!("Hello #{} from node A!", i).into_bytes());
        sleep(Duration::from_millis(100)).await;
    }

    sleep(Duration::from_secs(5)).await;

    let events = test.collect_events();

    assert_eq!(events.len(), N);

    for (i, (transactions, _)) in events.iter().enumerate() {
        if i == 0 || bi_nodes_to_kill.contains(&i) {
            // Node A
            continue;
        }

        assert_eq!(transactions.len(), R);
        let mut expected = (B - R - 1..B - 1)
            .map(|j| libp2p_dog::Transaction {
                from: peer_ids[0],
                seqno: 0, // ignored
                data: format!("Hello #{} from node A!", j).into_bytes(),
            })
            .collect::<Vec<_>>();

        for transaction in transactions {
            let index = match expected.iter().position(|expected| {
                expected.from == transaction.from && expected.data == transaction.data
            }) {
                Some(index) => index,
                None => panic!("Unexpected transaction: {:?}", transaction),
            };
            expected.remove(index);
        }
    }

    let mut exception = false;
    for (i, (_, routing_updates)) in events.iter().enumerate() {
        if i == 0 || bi_nodes_to_kill.contains(&i) || i == N - 1 {
            assert!(routing_updates.is_empty());
            continue;
        }

        match routing_updates.len() {
            0 => {
                if exception {
                    panic!("There can only be one exception");
                }
                exception = true;
            }
            1 => {
                assert_eq!(routing_updates[0].len(), 0);
            }
            n => panic!("Unexpected number of routing updates: {}", n),
        }
    }
}
