use crate::data::*;
use crate::fixtures::server::IsolatedServer;
use crate::fixtures::*;
use crate::setup::*;
use rebuilderd_common::api::v1::BuildRestApi;
use rstest::rstest;

#[rstest]
#[tokio::test]
pub async fn returns_no_result_for_empty_database(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    let result = client.get_build_log(1).await;

    assert!(result.is_err());
}

#[rstest]
#[tokio::test]
pub async fn returns_result_for_failed_build(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    setup_single_failed_rebuild(&client).await;

    let result = client.get_build_log(1).await.unwrap();

    assert_eq!(DUMMY_BUILD_LOG, result);
}

#[rstest]
#[tokio::test]
pub async fn returns_result_for_bad_build(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    setup_single_bad_rebuild(&client).await;

    let result = client.get_build_log(1).await.unwrap();

    assert_eq!(DUMMY_BUILD_LOG, result);
}
