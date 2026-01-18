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

    let results = client
        .get_distribution_components(DUMMY_DISTRIBUTION)
        .await
        .unwrap();
    assert!(results.is_empty());
}

#[rstest]
#[tokio::test]
pub async fn returns_correct_result_for_distribution_with_single_component(
    isolated_server: IsolatedServer,
) {
    let client = isolated_server.client;

    setup_single_imported_package(&client).await;

    let results = client
        .get_distribution_components(DUMMY_DISTRIBUTION)
        .await
        .unwrap();

    assert_eq!(1, results.len());
    assert_eq!(DUMMY_COMPONENT, results[0]);
}

#[rstest]
#[tokio::test]
pub async fn returns_correct_results_for_distribution_with_multiple_components(
    isolated_server: IsolatedServer,
) {
    let client = isolated_server.client;

    setup_single_imported_package(&client).await;
    client
        .submit_package_report(&single_package_report_from_different_component())
        .await
        .unwrap();

    let results = client
        .get_distribution_components(DUMMY_DISTRIBUTION)
        .await
        .unwrap();

    assert_eq!(2, results.len());

    assert!(results.contains(&DUMMY_COMPONENT.to_string()));
    assert!(results.contains(&DUMMY_OTHER_COMPONENT.to_string()));
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

    let result = client.get_distribution_components(DUMMY_DISTRIBUTION).await;

    assert!(result.is_ok());
}
