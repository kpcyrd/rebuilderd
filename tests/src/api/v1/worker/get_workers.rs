use crate::actions::*;
use crate::fixtures::server::IsolatedServer;
use crate::fixtures::*;
use rand::distr::{Alphanumeric, SampleString};
use rebuilderd_common::api::v1::WorkerRestApi;
use rstest::rstest;

#[rstest]
#[tokio::test]
pub async fn returns_no_results_for_empty_database(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    let results = client.get_workers(None).await.unwrap().records;

    assert!(results.is_empty());
}

#[rstest]
#[tokio::test]
pub async fn returns_single_result_for_database_with_single_worker(
    isolated_server: IsolatedServer,
) {
    let client = isolated_server.client;

    register_worker(&client).await;

    let results = client.get_workers(None).await.unwrap().records;

    assert_eq!(1, results.len());
}

#[rstest]
#[tokio::test]
pub async fn returns_multiple_results_for_database_with_multiple_workers(
    isolated_server: IsolatedServer,
) {
    let mut client = isolated_server.client;

    register_worker(&client).await;

    // create a new key for the new worker
    let worker_key = Alphanumeric.sample_string(&mut rand::rng(), 32);
    client.worker_key(worker_key);
    register_other_worker(&client).await;

    let results = client.get_workers(None).await.unwrap().records;

    assert_eq!(2, results.len());
}
