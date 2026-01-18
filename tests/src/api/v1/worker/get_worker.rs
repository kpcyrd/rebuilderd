use crate::actions::register_worker;
use crate::fixtures::server::IsolatedServer;
use crate::fixtures::*;
use rebuilderd_common::api::v1::WorkerRestApi;
use rstest::rstest;

#[rstest]
#[tokio::test]
pub async fn returns_no_results_for_empty_database(isolated_server: IsolatedServer) {
    let results = isolated_server.client.get_worker(1).await;

    assert!(results.is_err())
}

#[rstest]
#[tokio::test]
pub async fn returns_result_for_existing_id(isolated_server: IsolatedServer) {
    register_worker(&isolated_server.client).await;

    let results = isolated_server.client.get_worker(1).await;

    assert!(results.is_ok())
}

#[rstest]
#[tokio::test]
pub async fn returns_no_result_for_nonexistent_id(isolated_server: IsolatedServer) {
    register_worker(&isolated_server.client).await;

    let results = isolated_server.client.get_worker(99999).await;

    assert!(results.is_err())
}
