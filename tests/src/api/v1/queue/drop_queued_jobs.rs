use crate::actions::*;
use crate::assertions::assert_job_matches_package;
use crate::data::*;
use crate::fixtures::server::IsolatedServer;
use crate::fixtures::*;
use crate::setup::*;
use rebuilderd_common::api::v1::{
    IdentityFilter, OriginFilter, PackageReport, PackageRestApi, QueueRestApi,
};
use rstest::rstest;

#[rstest]
#[tokio::test]
pub async fn does_nothing_for_empty_database(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    client.drop_queued_jobs(None, None).await.unwrap();
}

#[rstest]
#[tokio::test]
pub async fn drops_all_jobs_if_no_filters_are_specified(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    import_multiple_packages(&client).await;

    client.drop_queued_jobs(None, None).await.unwrap();

    let jobs = client
        .get_queued_jobs(None, None, None)
        .await
        .unwrap()
        .records;

    assert!(jobs.is_empty());
}

#[rstest]
#[case(OriginFilter{
        distribution: Some(DUMMY_OTHER_DISTRIBUTION.to_string()),
        release: None,
        component: None,
        architecture: None,
    },
    single_package_report_from_different_distribution())]
#[case(OriginFilter {
        distribution: None,
        release: None,
        component: None,
        architecture: Some(DUMMY_OTHER_ARCHITECTURE.to_string()),
    }, single_package_report_from_different_architecture())]
#[tokio::test]
pub async fn drops_correct_job_for_matching_origin_filter(
    isolated_server: IsolatedServer,
    #[case] origin_filter: OriginFilter,
    #[case] extra_packages: PackageReport,
) {
    let client = isolated_server.client;

    let report = single_package_report();
    client.submit_package_report(&report).await.unwrap();
    client.submit_package_report(&extra_packages).await.unwrap();

    // make sure we have two jobs here so that we're not trying to test against
    // package friends, which don't get duplicate jobs
    let jobs = client
        .get_queued_jobs(None, None, None)
        .await
        .unwrap()
        .records;

    assert_eq!(2, jobs.len());

    client
        .drop_queued_jobs(Some(&origin_filter), None)
        .await
        .unwrap();

    let jobs = client
        .get_queued_jobs(None, None, None)
        .await
        .unwrap()
        .records;

    assert_eq!(1, jobs.len());

    assert_job_matches_package(&report, &report.packages[0], &jobs[0]);
}

#[rstest]
#[case(IdentityFilter{
        name: Some(DUMMY_SOURCE_PACKAGE.to_string()),
        version: None,
    })]
#[case(IdentityFilter{
        name: Some(DUMMY_MULTI_ARTIFACT_SOURCE_PACKAGE.to_string()),
        version: None,
    })]
#[case(IdentityFilter{
        name: None,
        version: Some(DUMMY_SOURCE_PACKAGE_VERSION.to_string()),
    })]
#[case(IdentityFilter{
        name: None,
        version: Some(DUMMY_MULTI_ARTIFACT_SOURCE_PACKAGE_VERSION.to_string()),
    })]
#[tokio::test]
pub async fn drops_correct_job_for_matching_identity_filter(
    isolated_server: IsolatedServer,
    #[case] identity_filter: IdentityFilter,
) {
    let client = isolated_server.client;

    setup_multiple_imported_packages(&client).await;

    client
        .drop_queued_jobs(None, Some(&identity_filter))
        .await
        .unwrap();

    let jobs = client
        .get_queued_jobs(None, None, None)
        .await
        .unwrap()
        .records;

    assert_eq!(1, jobs.len());

    let job = &jobs[0];

    if let Some(name) = identity_filter.name {
        assert_ne!(name, job.name);
    }

    if let Some(version) = identity_filter.version {
        assert_ne!(version, job.version);
    }
}
