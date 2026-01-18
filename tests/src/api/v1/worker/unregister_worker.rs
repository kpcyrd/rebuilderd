use crate::actions::*;
use crate::data::*;
use crate::fixtures::server::IsolatedServer;
use crate::fixtures::*;
use rand::distr::{Alphanumeric, SampleString};
use rebuilderd_common::api::v1::WorkerRestApi;
use rstest::rstest;

#[rstest]
#[tokio::test]
pub async fn unregisters_worker_correctly(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    register_worker(&client).await;

    client.unregister_worker(1).await.unwrap();

    let workers = client.get_workers(None).await.unwrap().records;

    assert!(workers.is_empty());
}

#[rstest]
#[tokio::test]
pub async fn does_not_unregister_other_workers(isolated_server: IsolatedServer) {
    let mut client = isolated_server.client;

    register_worker(&client).await;

    // create a new key for the new worker
    let worker_key = Alphanumeric.sample_string(&mut rand::rng(), 32);
    client.worker_key(worker_key);
    register_other_worker(&client).await;

    client.unregister_worker(1).await.unwrap();

    let workers = client.get_workers(None).await.unwrap().records;

    assert_eq!(1, workers.len());
    assert_eq!(DUMMY_OTHER_WORKER, workers[0].name);
}

#[rstest]
#[tokio::test]
pub async fn fails_if_worker_does_not_exist(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    register_worker(&client).await;

    let result = client.unregister_worker(9999).await;

    assert!(result.is_err());
}

#[rstest]
#[tokio::test]
pub async fn fails_if_no_worker_authentication_is_provided(isolated_server: IsolatedServer) {
    let mut client = isolated_server.client;

    register_worker(&client).await;

    // zero out key
    client.worker_key("");
    let result = client.unregister_worker(1).await;

    assert!(result.is_err());
}
