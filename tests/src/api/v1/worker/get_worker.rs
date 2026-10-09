use crate::actions::*;
use crate::fixtures::server::IsolatedServer;
use crate::fixtures::*;
use rebuilderd_common::api::v1::WorkerRestApi;
use rebuilderd_common::config::ConfigFile;
use rstest::rstest;
use std::time::Duration;

#[rstest]
#[tokio::test]
pub async fn returns_no_results_for_empty_database(mut isolated_server: IsolatedServer) {
    let results = isolated_server.client.get_worker(1).await;

    assert!(results.is_err());

    isolated_server.shutdown().await;
}

#[rstest]
#[tokio::test]
pub async fn returns_result_for_existing_id(mut isolated_server: IsolatedServer) {
    register_worker(&isolated_server.client).await;

    let results = isolated_server.client.get_worker(1).await;

    assert!(results.is_ok());

    isolated_server.shutdown().await;
}

#[rstest]
#[tokio::test]
pub async fn returns_no_result_for_nonexistent_id(mut isolated_server: IsolatedServer) {
    register_worker(&isolated_server.client).await;

    let results = isolated_server.client.get_worker(99999).await;

    assert!(results.is_err());

    isolated_server.shutdown().await;
}

#[rstest]
#[tokio::test]
pub async fn does_not_need_authentication(mut isolated_server: IsolatedServer) {
    let client = &mut isolated_server.client;

    register_worker(client).await;

    // zero out keys
    client.auth_cookie("");
    client.worker_key("");
    client.signup_secret("");

    let result = client.get_worker(1).await;

    assert!(result.is_ok());

    isolated_server.shutdown().await;
}

#[rstest]
#[tokio::test]
pub async fn reports_worker_past_offline_deadline_as_offline(
    #[with(None, None, None, Some(0))] config_file: ConfigFile,
    #[with(config_file.clone())] mut isolated_server: IsolatedServer,
) {
    let client = &isolated_server.client;
    let _config_file = config_file;

    register_worker(client).await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    let result = client.get_worker(1).await.unwrap();

    assert!(!result.is_online);

    isolated_server.shutdown().await;
}
