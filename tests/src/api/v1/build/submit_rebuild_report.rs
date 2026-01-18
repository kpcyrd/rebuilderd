use crate::actions::{import_single_package, pick_up_job, register_worker, report_bad_rebuild};
use crate::assertions::assert_job_matches_package;
use crate::data::*;
use crate::fixtures::server::IsolatedServer;
use crate::fixtures::*;
use crate::setup::*;
use chrono::Utc;
use rebuilderd_common::api::v1::{
    ArtifactStatus, BuildRestApi, BuildStatus, PackageRestApi, Priority, QueueRestApi,
};
use rebuilderd_common::config::ConfigFile;
use rstest::rstest;

#[rstest]
#[tokio::test]
pub async fn can_report_failed_rebuild(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    register_worker(&client).await;
    import_single_package(&client).await;

    let job = pick_up_job(&client).await;
    let report = failed_rebuild_report(&job);

    client.submit_build_report(report).await.unwrap();
}

#[rstest]
#[tokio::test]
pub async fn source_package_is_marked_failed_after_failed_report(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    setup_single_failed_rebuild(&client).await;

    let package = client
        .get_source_packages(None, None, None)
        .await
        .map(|p| p.records)
        .unwrap()
        .pop()
        .unwrap();

    assert_eq!(Some(BuildStatus::Fail), package.status)
}

#[rstest]
#[tokio::test]
pub async fn binary_package_is_marked_unknown_after_failed_report(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    setup_single_failed_rebuild(&client).await;

    let package = client
        .get_binary_packages(None, None, None)
        .await
        .map(|p| p.records)
        .unwrap()
        .pop()
        .unwrap();

    assert!(package.status.is_none_or(|v| v == ArtifactStatus::Unknown))
}

#[rstest]
#[tokio::test]
pub async fn package_is_requeued_after_failed_report(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    setup_single_failed_rebuild(&client).await;

    let jobs = client
        .get_queued_jobs(None, None, None)
        .await
        .unwrap()
        .records;

    assert_eq!(1, jobs.len());
}

#[rstest]
#[tokio::test]
pub async fn requeued_job_after_failed_report_has_correct_data(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    setup_single_failed_rebuild(&client).await;

    let package_report = single_package_report();
    let job = client
        .get_queued_jobs(None, None, None)
        .await
        .unwrap()
        .records
        .pop()
        .unwrap();

    let package = &package_report.packages[0];

    assert_job_matches_package(&package_report, package, &job);
    assert!(job.next_retry.unwrap() >= Utc::now().naive_utc());
    assert_eq!(Priority::retry(), job.priority);
    assert_eq!(None, job.started_at);
    assert!(job.queued_at <= Utc::now().naive_utc());
}

#[rstest]
#[tokio::test]
pub async fn can_report_bad_rebuild(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    register_worker(&client).await;
    import_single_package(&client).await;

    let job = pick_up_job(&client).await;
    let report = bad_rebuild_report(&job);

    client.submit_build_report(report).await.unwrap();
}

#[rstest]
#[tokio::test]
pub async fn source_package_is_marked_bad_after_bad_report(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    setup_single_bad_rebuild(&client).await;

    let package = client
        .get_source_packages(None, None, None)
        .await
        .map(|p| p.records)
        .unwrap()
        .pop()
        .unwrap();

    assert_eq!(Some(BuildStatus::Bad), package.status)
}

#[rstest]
#[tokio::test]
pub async fn binary_package_is_marked_bad_after_bad_report(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    setup_single_bad_rebuild(&client).await;

    let package = client
        .get_binary_packages(None, None, None)
        .await
        .map(|p| p.records)
        .unwrap()
        .pop()
        .unwrap();

    assert_eq!(Some(ArtifactStatus::Bad), package.status)
}

#[rstest]
#[tokio::test]
pub async fn package_is_requeued_after_bad_report(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    setup_single_bad_rebuild(&client).await;

    let jobs = client
        .get_queued_jobs(None, None, None)
        .await
        .unwrap()
        .records;

    assert_eq!(1, jobs.len());
}

#[rstest]
#[tokio::test]
pub async fn requeued_job_after_bad_report_has_correct_data(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    setup_single_bad_rebuild(&client).await;

    let package_report = single_package_report();
    let job = client
        .get_queued_jobs(None, None, None)
        .await
        .unwrap()
        .records
        .pop()
        .unwrap();

    let package = &package_report.packages[0];

    assert_job_matches_package(&package_report, package, &job);
    assert!(job.next_retry.unwrap() >= Utc::now().naive_utc());
    assert_eq!(Priority::retry(), job.priority);
    assert_eq!(None, job.started_at);
    assert!(job.queued_at <= Utc::now().naive_utc());
}

#[rstest]
#[tokio::test]
pub async fn package_is_not_requeued_if_max_retries_is_exceeded(
    #[with(None, Some(1), None)] config_file: ConfigFile,
    #[with(config_file.clone())] isolated_server: IsolatedServer,
) {
    let client = isolated_server.client;

    register_worker(&client).await;
    import_single_package(&client).await;
    report_bad_rebuild(&client).await;

    let jobs = client
        .get_queued_jobs(None, None, None)
        .await
        .unwrap()
        .records;

    assert!(jobs.is_empty());
}

#[rstest]
#[tokio::test]
pub async fn can_report_good_rebuild(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    register_worker(&client).await;
    import_single_package(&client).await;

    let job = pick_up_job(&client).await;
    let report = good_rebuild_report(&job);

    client.submit_build_report(report).await.unwrap();
}

#[rstest]
#[tokio::test]
pub async fn source_package_is_marked_good_after_good_report(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    setup_single_good_rebuild(&client).await;

    let package = client
        .get_source_packages(None, None, None)
        .await
        .map(|p| p.records)
        .unwrap()
        .pop()
        .unwrap();

    assert_eq!(Some(BuildStatus::Good), package.status)
}

#[rstest]
#[tokio::test]
pub async fn binary_package_is_marked_good_after_good_report(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    setup_single_good_rebuild(&client).await;

    let package = client
        .get_binary_packages(None, None, None)
        .await
        .map(|p| p.records)
        .unwrap()
        .pop()
        .unwrap();

    assert_eq!(Some(ArtifactStatus::Good), package.status)
}

#[rstest]
#[tokio::test]
pub async fn package_is_not_requeued_after_good_report(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    setup_single_good_rebuild(&client).await;

    let jobs = client
        .get_queued_jobs(None, None, None)
        .await
        .unwrap()
        .records;

    assert!(jobs.is_empty())
}

#[rstest]
#[tokio::test]
pub async fn can_report_good_rebuild_with_signed_attestation(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    register_worker(&client).await;
    import_single_package(&client).await;

    let job = pick_up_job(&client).await;
    let report = good_rebuild_report_with_signed_attestation(&job).await;

    client.submit_build_report(report).await.unwrap();
}

#[rstest]
#[tokio::test]
pub async fn can_report_good_rebuild_with_unsigned_attestation(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    register_worker(&client).await;
    import_single_package(&client).await;

    let job = pick_up_job(&client).await;
    let report = good_rebuild_report_with_unsigned_attestation(&job).await;

    client.submit_build_report(report).await.unwrap();
}
