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
