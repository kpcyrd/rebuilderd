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
        .get_distribution_releases(DUMMY_DISTRIBUTION)
        .await
        .unwrap();
    assert!(results.is_empty());
}

#[rstest]
#[tokio::test]
pub async fn returns_correct_result_for_distribution_with_single_release(
    isolated_server: IsolatedServer,
) {
    let client = isolated_server.client;

    setup_single_imported_package(&client).await;

    let results = client
        .get_distribution_releases(DUMMY_DISTRIBUTION)
        .await
        .unwrap();

    assert_eq!(1, results.len());
    assert_eq!(DUMMY_RELEASE, results[0]);
}

#[rstest]
#[tokio::test]
pub async fn returns_correct_results_for_distribution_with_multiple_releases(
    isolated_server: IsolatedServer,
) {
    let client = isolated_server.client;

    setup_single_imported_package(&client).await;
    client
        .submit_package_report(&single_package_report_from_different_release())
        .await
        .unwrap();

    let results = client
        .get_distribution_releases(DUMMY_DISTRIBUTION)
        .await
        .unwrap();

    assert_eq!(2, results.len());

    assert!(results.contains(&DUMMY_RELEASE.to_string()));
    assert!(results.contains(&DUMMY_OTHER_RELEASE.to_string()));
}
