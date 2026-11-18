use crate::data::*;
use crate::fixtures::server::IsolatedServer;
use crate::fixtures::*;
use crate::setup;
use rebuilderd_common::api::v1::{DashboardFilter, DashboardRestApi, PackageRestApi};
use rstest::rstest;

fn dashboard_filter() -> DashboardFilter {
    DashboardFilter {
        distribution: DUMMY_DISTRIBUTION.to_string(),
        release: DUMMY_RELEASE.to_string(),
        component: None,
        architecture: DUMMY_ARCHITECTURE.to_string(),
    }
}

#[rstest]
#[tokio::test]
pub async fn returns_zero_sums_for_empty_database(mut isolated_server: IsolatedServer) {
    let client = &isolated_server.client;

    let result = client.get_dashboard(&dashboard_filter()).await.unwrap();

    assert_eq!(0, result.rebuilds.bad);
    assert_eq!(0, result.rebuilds.fail);
    assert_eq!(0, result.rebuilds.good);
    assert_eq!(0, result.rebuilds.unknown);

    isolated_server.shutdown().await;
}

#[rstest]
#[tokio::test]
pub async fn returns_correct_sums_for_database_with_unbuilt_package(
    mut isolated_server: IsolatedServer,
) {
    let client = &isolated_server.client;

    setup::single_imported_package(client).await;

    let result = client.get_dashboard(&dashboard_filter()).await.unwrap();

    assert_eq!(0, result.rebuilds.bad);
    assert_eq!(0, result.rebuilds.fail);
    assert_eq!(0, result.rebuilds.good);
    assert_eq!(1, result.rebuilds.unknown);

    isolated_server.shutdown().await;
}

#[rstest]
#[tokio::test]
pub async fn filters_build_input_and_counts_multiple_artifacts_once(
    mut isolated_server: IsolatedServer,
) {
    let client = &isolated_server.client;
    let mut report = single_package_with_multiple_artifacts_report();
    report.packages[0].artifacts[1].architecture = DUMMY_OTHER_ARCHITECTURE.to_string();
    client.submit_package_report(&report).await.unwrap();

    for (distribution, architecture, release, component, expected) in [
        (
            DUMMY_DISTRIBUTION,
            DUMMY_ARCHITECTURE,
            DUMMY_RELEASE,
            None,
            1,
        ),
        (
            DUMMY_DISTRIBUTION,
            DUMMY_ARCHITECTURE,
            DUMMY_RELEASE,
            Some(DUMMY_COMPONENT),
            1,
        ),
        (
            DUMMY_DISTRIBUTION,
            DUMMY_ARCHITECTURE,
            DUMMY_RELEASE,
            Some(DUMMY_OTHER_COMPONENT),
            0,
        ),
        (
            DUMMY_DISTRIBUTION,
            DUMMY_ARCHITECTURE,
            DUMMY_OTHER_RELEASE,
            None,
            0,
        ),
        (
            DUMMY_DISTRIBUTION,
            DUMMY_OTHER_ARCHITECTURE,
            DUMMY_RELEASE,
            None,
            0,
        ),
        (
            DUMMY_OTHER_DISTRIBUTION,
            DUMMY_ARCHITECTURE,
            DUMMY_RELEASE,
            None,
            0,
        ),
    ] {
        let filter = DashboardFilter {
            distribution: distribution.to_string(),
            release: release.to_string(),
            component: component.map(str::to_string),
            architecture: architecture.to_string(),
        };
        let dashboard = client.get_dashboard(&filter).await.unwrap();
        assert_eq!(expected, dashboard.rebuilds.unknown);
        assert_eq!(expected, dashboard.jobs.available);
    }

    isolated_server.shutdown().await;
}

#[rstest]
#[tokio::test]
pub async fn excludes_null_release_when_filtered(mut isolated_server: IsolatedServer) {
    let client = &isolated_server.client;
    setup::single_imported_package_with_null_release(client).await;

    let filtered = client.get_dashboard(&dashboard_filter()).await.unwrap();
    assert_eq!(0, filtered.rebuilds.unknown);

    isolated_server.shutdown().await;
}

#[rstest]
#[tokio::test]
pub async fn returns_correct_sums_for_database_with_good_package(
    mut isolated_server: IsolatedServer,
) {
    let client = &isolated_server.client;

    setup::single_good_rebuild(client).await;

    let result = client.get_dashboard(&dashboard_filter()).await.unwrap();

    assert_eq!(0, result.rebuilds.bad);
    assert_eq!(0, result.rebuilds.fail);
    assert_eq!(1, result.rebuilds.good);
    assert_eq!(0, result.rebuilds.unknown);

    isolated_server.shutdown().await;
}

#[rstest]
#[tokio::test]
pub async fn returns_correct_sums_for_database_with_bad_package(
    mut isolated_server: IsolatedServer,
) {
    let client = &isolated_server.client;

    setup::single_bad_rebuild(client).await;

    let result = client.get_dashboard(&dashboard_filter()).await.unwrap();

    assert_eq!(1, result.rebuilds.bad);
    assert_eq!(0, result.rebuilds.fail);
    assert_eq!(0, result.rebuilds.good);
    assert_eq!(0, result.rebuilds.unknown);

    isolated_server.shutdown().await;
}

#[rstest]
#[tokio::test]
pub async fn returns_correct_sums_for_database_with_failed_package(
    mut isolated_server: IsolatedServer,
) {
    let client = &isolated_server.client;

    setup::single_failed_rebuild(client).await;

    let result = client.get_dashboard(&dashboard_filter()).await.unwrap();

    assert_eq!(0, result.rebuilds.bad);
    assert_eq!(1, result.rebuilds.fail);
    assert_eq!(0, result.rebuilds.good);
    assert_eq!(0, result.rebuilds.unknown);

    isolated_server.shutdown().await;
}

#[rstest]
#[tokio::test]
pub async fn returns_zero_job_counts_for_empty_database(mut isolated_server: IsolatedServer) {
    let client = &isolated_server.client;

    let result = client.get_dashboard(&dashboard_filter()).await.unwrap();

    assert_eq!(0, result.jobs.available);
    assert_eq!(0, result.jobs.pending);
    assert_eq!(0, result.jobs.running);

    isolated_server.shutdown().await;
}

#[rstest]
#[tokio::test]
pub async fn returns_correct_job_counts_for_unbuilt_package(mut isolated_server: IsolatedServer) {
    let client = &isolated_server.client;

    setup::single_imported_package(client).await;

    let result = client.get_dashboard(&dashboard_filter()).await.unwrap();

    assert_eq!(1, result.jobs.available);
    assert_eq!(0, result.jobs.pending);
    assert_eq!(0, result.jobs.running);

    isolated_server.shutdown().await;
}

#[rstest]
#[tokio::test]
pub async fn returns_correct_job_counts_for_failed_package(mut isolated_server: IsolatedServer) {
    let client = &isolated_server.client;

    setup::single_bad_rebuild(client).await;

    let result = client.get_dashboard(&dashboard_filter()).await.unwrap();

    assert_eq!(0, result.jobs.available);
    assert_eq!(1, result.jobs.pending);
    assert_eq!(0, result.jobs.running);

    isolated_server.shutdown().await;
}

#[rstest]
#[tokio::test]
pub async fn returns_correct_jobs_counts_for_package_in_progress(
    mut isolated_server: IsolatedServer,
) {
    let client = &isolated_server.client;

    setup::single_rebuild_in_progress(client).await;

    let result = client.get_dashboard(&dashboard_filter()).await.unwrap();

    assert_eq!(0, result.jobs.available);
    assert_eq!(0, result.jobs.pending);
    assert_eq!(1, result.jobs.running);

    isolated_server.shutdown().await;
}

#[rstest]
#[tokio::test]
pub async fn does_not_need_authentication(mut isolated_server: IsolatedServer) {
    let client = &mut isolated_server.client;

    setup::single_imported_package(client).await;

    // zero out keys
    client.auth_cookie("");
    client.worker_key("");
    client.signup_secret("");

    let result = client.get_dashboard(&dashboard_filter()).await;

    assert!(result.is_ok());

    isolated_server.shutdown().await;
}
