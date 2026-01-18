use crate::fixtures::server::IsolatedServer;
use crate::fixtures::*;
use crate::setup::*;
use rebuilderd_common::api::v1::BuildRestApi;
use rstest::rstest;

#[rstest]
#[tokio::test]
pub async fn returns_no_results_for_empty_database(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    let results = client.get_build_artifact(1, 1).await;

    assert!(results.is_err())
}

#[rstest]
#[tokio::test]
pub async fn returns_result_for_existing_id(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    setup_single_good_rebuild(&client).await;

    let results = client.get_build_artifact(1, 1).await;

    assert!(results.is_ok())
}

#[rstest]
#[tokio::test]
pub async fn returns_no_result_for_nonexistent_build_id(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    setup_single_good_rebuild(&client).await;

    let results = client.get_build_artifact(99999, 1).await;

    assert!(results.is_err())
}

#[rstest]
#[tokio::test]
pub async fn returns_no_result_for_nonexistent_artifact_id(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    setup_single_good_rebuild(&client).await;

    let results = client.get_build_artifact(1, 99999).await;

    assert!(results.is_err())
}

#[rstest]
#[tokio::test]
pub async fn does_not_need_authentication(isolated_server: IsolatedServer) {
    let mut client = isolated_server.client;

    setup_single_good_rebuild(&client).await;

    // zero out keys
    client.auth_cookie("");
    client.worker_key("");
    client.signup_secret("");

    let result = client.get_build_artifact(1, 1).await;

    assert!(result.is_ok());
}
