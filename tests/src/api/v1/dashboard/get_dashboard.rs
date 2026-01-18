use crate::fixtures::server::IsolatedServer;
use crate::fixtures::*;
use crate::setup::*;
use rebuilderd_common::api::v1::DashboardRestApi;
use rstest::rstest;

#[rstest]
#[tokio::test]
pub async fn returns_zero_sums_for_empty_database(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    let result = client.get_dashboard(None).await.unwrap();

    assert_eq!(0, result.rebuilds.bad);
    assert_eq!(0, result.rebuilds.fail);
    assert_eq!(0, result.rebuilds.good);
    assert_eq!(0, result.rebuilds.unknown);
}

#[rstest]
#[tokio::test]
pub async fn returns_correct_sums_for_database_with_unbuilt_package(
    isolated_server: IsolatedServer,
) {
    let client = isolated_server.client;

    setup_single_imported_package(&client).await;

    let result = client.get_dashboard(None).await.unwrap();

    assert_eq!(0, result.rebuilds.bad);
    assert_eq!(0, result.rebuilds.fail);
    assert_eq!(0, result.rebuilds.good);
    assert_eq!(1, result.rebuilds.unknown);
}

#[rstest]
#[tokio::test]
pub async fn returns_correct_sums_for_database_with_good_package(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    setup_single_good_rebuild(&client).await;

    let result = client.get_dashboard(None).await.unwrap();

    assert_eq!(0, result.rebuilds.bad);
    assert_eq!(0, result.rebuilds.fail);
    assert_eq!(1, result.rebuilds.good);
    assert_eq!(0, result.rebuilds.unknown);
}

#[rstest]
#[tokio::test]
pub async fn returns_correct_sums_for_database_with_bad_package(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    setup_single_bad_rebuild(&client).await;

    let result = client.get_dashboard(None).await.unwrap();

    assert_eq!(1, result.rebuilds.bad);
    assert_eq!(0, result.rebuilds.fail);
    assert_eq!(0, result.rebuilds.good);
    assert_eq!(0, result.rebuilds.unknown);
}

#[rstest]
#[tokio::test]
pub async fn returns_correct_sums_for_database_with_failed_package(
    isolated_server: IsolatedServer,
) {
    let client = isolated_server.client;

    setup_single_failed_rebuild(&client).await;

    let result = client.get_dashboard(None).await.unwrap();

    assert_eq!(0, result.rebuilds.bad);
    assert_eq!(1, result.rebuilds.fail);
    assert_eq!(0, result.rebuilds.good);
    assert_eq!(0, result.rebuilds.unknown);
}

#[rstest]
#[tokio::test]
pub async fn returns_zero_job_counts_for_empty_database(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    let result = client.get_dashboard(None).await.unwrap();

    assert_eq!(0, result.jobs.available);
    assert_eq!(0, result.jobs.pending);
    assert_eq!(0, result.jobs.running);
}

#[rstest]
#[tokio::test]
pub async fn returns_correct_job_counts_for_unbuilt_package(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    setup_single_imported_package(&client).await;

    let result = client.get_dashboard(None).await.unwrap();

    assert_eq!(1, result.jobs.available);
    assert_eq!(0, result.jobs.pending);
    assert_eq!(0, result.jobs.running);
}

#[rstest]
#[tokio::test]
pub async fn returns_correct_job_counts_for_failed_package(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    setup_single_bad_rebuild(&client).await;

    let result = client.get_dashboard(None).await.unwrap();

    assert_eq!(0, result.jobs.available);
    assert_eq!(1, result.jobs.pending);
    assert_eq!(0, result.jobs.running);
}

#[rstest]
#[tokio::test]
pub async fn returns_correct_jobs_counts_for_package_in_progress(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    setup_single_rebuild_in_progress(&client).await;

    let result = client.get_dashboard(None).await.unwrap();

    assert_eq!(0, result.jobs.available);
    assert_eq!(0, result.jobs.pending);
    assert_eq!(1, result.jobs.running);
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

    let result = client.get_dashboard(None).await;

    assert!(result.is_ok());
}
