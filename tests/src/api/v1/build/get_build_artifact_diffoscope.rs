use crate::data::*;
use crate::fixtures::server::IsolatedServer;
use crate::fixtures::*;
use crate::setup;
use rebuilderd_common::api::v1::BuildRestApi;
use rstest::rstest;

#[rstest]
#[tokio::test]
pub async fn returns_no_result_for_empty_database(mut isolated_server: IsolatedServer) {
    let client = &isolated_server.client;

    let result = client.get_build_artifact_diffoscope(1, 1).await;

    assert!(result.is_err());

    isolated_server.shutdown().await;
}

#[rstest]
#[tokio::test]
pub async fn returns_no_result_for_failed_build(mut isolated_server: IsolatedServer) {
    let client = &isolated_server.client;

    setup::single_failed_rebuild(client).await;

    let result = client.get_build_artifact_diffoscope(1, 1).await;

    assert!(result.is_err());

    isolated_server.shutdown().await;
}

#[rstest]
#[tokio::test]
pub async fn returns_result_for_bad_build(mut isolated_server: IsolatedServer) {
    let client = &isolated_server.client;

    setup::single_bad_rebuild(client).await;

    let result = client.get_build_artifact_diffoscope(1, 1).await.unwrap();

    assert_eq!(DUMMY_DIFFOSCOPE, result);

    isolated_server.shutdown().await;
}

#[rstest]
#[tokio::test]
pub async fn does_not_need_authentication(mut isolated_server: IsolatedServer) {
    let client = &mut isolated_server.client;

    setup::single_bad_rebuild(client).await;

    // zero out keys
    client.auth_cookie("");
    client.worker_key("");
    client.signup_secret("");

    let result = client.get_build_artifact_diffoscope(1, 1).await;

    assert!(result.is_ok());

    isolated_server.shutdown().await;
}
