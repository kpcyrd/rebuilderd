use crate::actions::*;
use crate::data::*;
use crate::fixtures::server::IsolatedServer;
use crate::fixtures::*;
use crate::setup::*;
use rebuilderd_common::api::v1::BuildRestApi;
use rstest::rstest;

#[rstest]
#[tokio::test]
pub async fn returns_no_results_for_empty_database(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    let results = client.get_build_artifacts(1).await.unwrap();

    assert!(results.is_empty())
}

#[rstest]
#[tokio::test]
pub async fn returns_no_results_for_failed_build(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    setup_single_failed_rebuild(&client).await;

    let results = client.get_build_artifacts(1).await.unwrap();

    assert!(results.is_empty())
}

#[rstest]
#[tokio::test]
pub async fn returns_correct_results_for_good_build(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    import_single_package(&client).await;
    register_worker(&client).await;
    report_good_rebuild(&client).await;

    let results = client.get_build_artifacts(1).await.unwrap();

    assert_eq!(1, results.len());

    let artifact = &results[0];
    assert_eq!(DUMMY_BINARY_PACKAGE, artifact.name);
}

#[rstest]
#[tokio::test]
pub async fn returns_correct_results_for_good_build_with_multiple_artifacts(
    isolated_server: IsolatedServer,
) {
    let client = isolated_server.client;

    import_single_package_with_multiple_artifacts(&client).await;
    register_worker(&client).await;
    report_good_rebuild(&client).await;

    let results = client.get_build_artifacts(1).await.unwrap();

    assert_eq!(2, results.len());

    let artifact_1 = &results[0];
    let artifact_2 = &results[1];
    assert_eq!(DUMMY_MULTI_ARTIFACT_BINARY_PACKAGE_1, artifact_1.name);
    assert_eq!(DUMMY_MULTI_ARTIFACT_BINARY_PACKAGE_2, artifact_2.name);
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

    let result = client.get_build_artifacts(1).await;

    assert!(result.is_ok());
}
