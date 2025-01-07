use std::time::Duration;

use libp2p_dog::Route;
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

    let mut test = match Test::<2>::new_with_unique_config(config, bootstrap_sets) {
        Ok(test) => test,
        Err(e) => panic!("Failed to create test: {}", e),
    };

    test.spawn_all().await;

    for i in 0..10 {
        test.publish_on_node(0, format!("Hello #{} from node 1!", i).into_bytes());
        test.publish_on_node(1, format!("Hello #{} from node 2!", i).into_bytes());
    }

    sleep(Duration::from_secs(2)).await;

    let peer_ids = test.peer_ids();
    let events = test.collect_events();

    assert_eq!(events.len(), 2);

    for (i, (transactions, routes)) in events.iter().enumerate() {
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

        assert_eq!(routes.len(), 0);
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

    let mut test = match Test::<N>::new_with_unique_config(config, bootstrap_sets) {
        Ok(test) => test,
        Err(e) => panic!("Failed to create test: {}", e),
    };

    test.spawn_all().await;

    for i in 0..10 {
        for j in 0..N {
            test.publish_on_node(j, format!("Hello #{} from node {}!", i, j).into_bytes());
        }
    }

    sleep(Duration::from_secs(2)).await;

    let peer_ids = test.peer_ids();
    let events = test.collect_events();

    assert_eq!(events.len(), N);

    for (i, (transactions, routes)) in events.iter().enumerate() {
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
            let index = match expected
                .iter()
                .position(|expected| expected.from == transaction.from)
            {
                Some(index) => index,
                None => panic!("Wrong transaction: {:?}", transaction),
            };
            assert_eq!(transaction.data, expected[index].data);
            expected.remove(index);
        }

        assert_eq!(routes.len(), 0);
    }
}

// Testing that a node receiving the same transaction from different nodes will request one of them
// to stop sending it.
// We consider the following scenario:
//
//   1
//  / \
// 0   2
//  \ /
//   3
//
// We start by publishing a single transaction from each node. Consequently, each node will receive
// one of the transactions twice. For example, node 3 will receive the transaction originated from
// node 1 twice: via node 0 and node 2.
// We expect all nodes to request one of its neighbors to stop sending transactions that have the
// corresponding transaction's origin.
#[tokio::test]
pub async fn simple_redundancy() {
    let config = libp2p_dog::ConfigBuilder::default()
        // We force the nodes to send an have_tx message to stop the redundancy
        .target_redundancy(0.0)
        .redundancy_delta_percent(0)
        .build()
        .unwrap();

    const N: usize = 4;

    let bootstrap_sets: [Vec<usize>; N] = [vec![1, 3], vec![0, 2], vec![1, 3], vec![0, 2]];

    let mut test = match Test::<N>::new_with_unique_config(config, bootstrap_sets) {
        Ok(test) => test,
        Err(e) => panic!("Failed to create test: {}", e),
    };

    test.spawn_all().await;

    for i in 0..N {
        test.publish_on_node(i, format!("Hello from node {}!", i).into_bytes());
    }

    sleep(Duration::from_secs(2)).await;

    let peer_ids = test.peer_ids();
    let events = test.collect_events();

    assert_eq!(events.len(), N);

    for (i, (transactions, _)) in events.iter().enumerate() {
        assert_eq!(transactions.len(), N - 1);
        let expected = (0..N)
            .filter(|j| *j != i)
            .map(|j| libp2p_dog::Transaction {
                from: peer_ids[j],
                seqno: 0, // ignored
                data: format!("Hello from node {}!", j).into_bytes(),
            })
            .collect::<Vec<_>>();

        for transaction in transactions {
            let expected_transaction = expected
                .iter()
                .find(|expected| expected.from == transaction.from)
                .unwrap();
            assert_eq!(transaction.data, expected_transaction.data);
        }
    }

    let routes_0 = events[0].1.clone();
    let routes_2 = events[2].1.clone();

    // There should be two routing updates made of a single route
    assert_eq!(routes_0.len() + routes_2.len(), 2);

    let peer_id_to_index = |peer_id: &libp2p::PeerId| -> usize {
        peer_ids.iter().position(|id| id == peer_id).unwrap()
    };

    // There can be two valid behaviours:
    // 1. Each node has one of the disabled routes, but not the same one.
    // 2. One node has both disabled routes, which means that there are two events:
    //    - One with the disabled route from 1 to 3 or from 3 to 1.
    //    - A second one with the previous route and the inverse one.
    if routes_0.len() == 2 {
        assert_eq!(routes_0[0].len(), 1);
        let first_route = routes_0[0][0];

        assert_eq!(routes_0[1].len(), 2);
        assert_eq!(routes_0[1][0], first_route);
        assert_eq!(
            routes_0[1][1],
            Route::new(*first_route.target(), *first_route.source())
        );

        assert!(
            peer_id_to_index(first_route.source()) == 1
                || peer_id_to_index(first_route.source()) == 3
        );
        if peer_id_to_index(first_route.source()) == 1 {
            assert_eq!(peer_id_to_index(first_route.target()), 3);
        } else {
            assert_eq!(peer_id_to_index(first_route.target()), 1);
        }
    } else if routes_2.len() == 2 {
        assert_eq!(routes_2[0].len(), 1);
        let first_route = routes_2[0][0];

        assert_eq!(routes_2[1].len(), 2);
        assert_eq!(routes_2[1][0], first_route);
        assert_eq!(
            routes_2[1][1],
            Route::new(*first_route.target(), *first_route.source())
        );

        assert!(
            peer_id_to_index(first_route.source()) == 1
                || peer_id_to_index(first_route.source()) == 3
        );
        if peer_id_to_index(first_route.source()) == 1 {
            assert_eq!(peer_id_to_index(first_route.target()), 3);
        } else {
            assert_eq!(peer_id_to_index(first_route.target()), 1);
        }
    } else {
        assert_eq!(routes_0.len(), 1);
        assert_eq!(routes_2.len(), 1);

        let route_0 = routes_0[0][0];
        let route_2 = routes_2[0][0];

        assert_eq!(route_0.source(), route_2.target());
        assert_eq!(route_0.target(), route_2.source());

        assert!(peer_id_to_index(route_0.source()) == 1 || peer_id_to_index(route_0.source()) == 3);
        if peer_id_to_index(route_0.source()) == 1 {
            assert_eq!(peer_id_to_index(route_0.target()), 3);
        } else {
            assert_eq!(peer_id_to_index(route_0.target()), 1);
        }
    }

    let routes_1 = events[1].1.clone();
    let routes_3 = events[3].1.clone();

    assert_eq!(routes_1.len() + routes_3.len(), 2);

    if routes_1.len() == 2 {
        assert_eq!(routes_1[0].len(), 1);
        let first_route = routes_1[0][0];

        assert_eq!(routes_1[1].len(), 2);
        assert_eq!(routes_1[1][0], first_route);
        assert_eq!(
            routes_1[1][1],
            Route::new(*first_route.target(), *first_route.source())
        );

        assert!(
            peer_id_to_index(first_route.source()) == 0
                || peer_id_to_index(first_route.source()) == 2
        );
        if peer_id_to_index(first_route.source()) == 0 {
            assert_eq!(peer_id_to_index(first_route.target()), 2);
        } else {
            assert_eq!(peer_id_to_index(first_route.target()), 0);
        }
    } else if routes_3.len() == 2 {
        assert_eq!(routes_3[0].len(), 1);
        let first_route = routes_3[0][0];

        assert_eq!(routes_3[1].len(), 2);
        assert_eq!(routes_3[1][0], first_route);
        assert_eq!(
            routes_3[1][1],
            Route::new(*first_route.target(), *first_route.source())
        );

        assert!(
            peer_id_to_index(first_route.source()) == 0
                || peer_id_to_index(first_route.source()) == 2
        );
        if peer_id_to_index(first_route.source()) == 0 {
            assert_eq!(peer_id_to_index(first_route.target()), 2);
        } else {
            assert_eq!(peer_id_to_index(first_route.target()), 0);
        }
    } else {
        assert_eq!(routes_1.len(), 1);
        assert_eq!(routes_3.len(), 1);

        let route_1 = routes_1[0][0];
        let route_3 = routes_3[0][0];

        assert_eq!(route_1.source(), route_3.target());
        assert_eq!(route_1.target(), route_3.source());

        assert!(peer_id_to_index(route_1.source()) == 0 || peer_id_to_index(route_1.source()) == 2);
        if peer_id_to_index(route_1.source()) == 0 {
            assert_eq!(peer_id_to_index(route_1.target()), 2);
        } else {
            assert_eq!(peer_id_to_index(route_1.target()), 0);
        }
    }
}
