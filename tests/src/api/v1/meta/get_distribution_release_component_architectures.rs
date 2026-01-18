use crate::data::*;
use crate::fixtures::server::IsolatedServer;
use crate::fixtures::*;
use crate::setup::setup_single_imported_package;
use rebuilderd_common::api::v1::{MetaRestApi, PackageRestApi};
use rstest::rstest;

#[rstest]
#[tokio::test]
pub async fn returns_no_results_for_empty_database(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    let results = client.get_distributions().await.unwrap();
    assert!(results.is_empty());
}

#[rstest]
#[tokio::test]
pub async fn returns_correct_result_for_component_with_single_architecture(
    isolated_server: IsolatedServer,
) {
    let client = isolated_server.client;

    setup_single_imported_package(&client).await;

    let results = client
        .get_distribution_release_component_architectures(
            DUMMY_DISTRIBUTION,
            DUMMY_RELEASE,
            DUMMY_COMPONENT,
        )
        .await
        .unwrap();

    assert_eq!(1, results.len());
    assert_eq!(DUMMY_ARCHITECTURE, results[0]);
}

#[rstest]
#[tokio::test]
pub async fn returns_correct_results_for_component_with_multiple_architectures(
    isolated_server: IsolatedServer,
) {
    let client = isolated_server.client;

    setup_single_imported_package(&client).await;
    client
        .submit_package_report(&single_package_report_from_different_architecture())
        .await
        .unwrap();

    let results = client
        .get_distribution_release_component_architectures(
            DUMMY_DISTRIBUTION,
            DUMMY_RELEASE,
            DUMMY_COMPONENT,
        )
        .await
        .unwrap();

    assert_eq!(2, results.len());

    assert!(results.contains(&DUMMY_ARCHITECTURE.to_string()));
    assert!(results.contains(&DUMMY_OTHER_ARCHITECTURE.to_string()));
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

    let result = client
        .get_distribution_release_component_architectures(
            DUMMY_DISTRIBUTION,
            DUMMY_RELEASE,
            DUMMY_COMPONENT,
        )
        .await;

    assert!(result.is_ok());
}
