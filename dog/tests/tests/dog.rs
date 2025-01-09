use std::time::Duration;

use libp2p_dog_tests::Test;
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
