use crate::fixtures::server::IsolatedServer;
use crate::fixtures::*;
use crate::setup::*;
use rebuilderd_common::api::v1::PackageRestApi;
use rstest::rstest;

#[rstest]
#[tokio::test]
pub async fn returns_no_results_for_empty_database(isolated_server: IsolatedServer) {
    let results = isolated_server.client.get_binary_package(1).await;

    assert!(results.is_err())
}

#[rstest]
#[tokio::test]
pub async fn returns_result_for_existing_id(isolated_server: IsolatedServer) {
    setup_single_imported_package(&isolated_server.client).await;

    let results = isolated_server.client.get_binary_package(1).await;

    assert!(results.is_ok())
}

#[rstest]
#[tokio::test]
pub async fn returns_no_result_for_nonexistent_id(isolated_server: IsolatedServer) {
    setup_single_imported_package(&isolated_server.client).await;

    let results = isolated_server.client.get_binary_package(99999).await;

    assert!(results.is_err())
}

#[rstest]
#[tokio::test]
pub async fn does_not_need_authentication(isolated_server: IsolatedServer) {
    let mut client = isolated_server.client;

    setup_single_imported_package(&client).await;

    // zero out keys
    client.auth_cookie("");
    client.worker_key("");
    client.signup_secret("");

    let result = client.get_binary_package(1).await;

    assert!(result.is_ok());
}
