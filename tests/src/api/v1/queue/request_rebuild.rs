use crate::actions::*;
use crate::data::*;
use crate::fixtures::server::IsolatedServer;
use crate::fixtures::*;
use crate::setup::*;
use chrono::Utc;
use rebuilderd_common::api::v1::{
    BuildStatus, PackageReport, PackageRestApi, Priority, QueueJobRequest, QueueRestApi,
};
use rebuilderd_common::config::ConfigFile;
use rstest::rstest;

#[rstest]
#[tokio::test]
pub async fn can_requeue_bad_packages(mut isolated_server: IsolatedServer) {
    let client = &isolated_server.client;

    register_worker(client).await;
    import_single_package(client).await;
    report_bad_rebuild(client).await;

    client
        .request_rebuild(QueueJobRequest {
            distribution: None,
            release: None,
            component: None,
            name: None,
            version: None,
            architecture: None,
            status: Some(BuildStatus::Bad),
            priority: Some(Priority::default()),
        })
        .await
        .unwrap();

    isolated_server.shutdown().await;
}

#[rstest]
#[tokio::test]
pub async fn requeued_packages_are_due_instantly(mut isolated_server: IsolatedServer) {
    let client = &isolated_server.client;

    setup_single_rebuild_request(client).await;

    let job = client
        .get_queued_jobs(None, None, None)
        .await
        .unwrap()
        .records
        .pop()
        .unwrap();

    assert!(job.next_retry.unwrap() <= Utc::now().naive_utc());

    isolated_server.shutdown().await;
}

#[rstest]
#[tokio::test]
pub async fn requeued_packages_are_queued_with_manual_priority_by_default(
    mut isolated_server: IsolatedServer,
) {
    let client = &isolated_server.client;

    setup_single_rebuild_request(client).await;

    let job = client
        .get_queued_jobs(None, None, None)
        .await
        .unwrap()
        .records
        .pop()
        .unwrap();

    assert_eq!(Priority::manual(), job.priority);

    isolated_server.shutdown().await;
}

#[rstest]
#[tokio::test]
pub async fn can_update_job_priority(mut isolated_server: IsolatedServer) {
    let client = &isolated_server.client;

    setup_single_rebuild_request(client).await;

    client
        .request_rebuild(QueueJobRequest {
            distribution: None,
            release: None,
            component: None,
            name: None,
            version: None,
            architecture: None,
            status: None,
            priority: Some(Priority::manual()),
        })
        .await
        .unwrap();

    let job = client
        .get_queued_jobs(None, None, None)
        .await
        .unwrap()
        .records
        .pop()
        .unwrap();

    assert_eq!(Priority::manual(), job.priority);

    isolated_server.shutdown().await;
}

#[rstest]
#[tokio::test]
pub async fn fails_if_no_admin_authentication_is_provided(mut isolated_server: IsolatedServer) {
    let client = &mut isolated_server.client;

    // zero out key
    client.auth_cookie("");
    let result = client
        .request_rebuild(QueueJobRequest {
            distribution: None,
            release: None,
            component: None,
            name: None,
            version: None,
            architecture: None,
            status: None,
            priority: Some(Priority::manual()),
        })
        .await;

    assert!(result.is_err());

    isolated_server.shutdown().await;
}

#[rstest]
#[case(single_package_report_from_different_release())]
#[case(single_package_report_from_different_component())]
#[tokio::test]
pub async fn does_not_requeue_friends(
    mut isolated_server: IsolatedServer,
    #[case] extra_packages: PackageReport,
) {
    let client = &isolated_server.client;

    // first, a single package with a bad rebuild
    setup_single_bad_rebuild(client).await;

    // then, the friend of that package
    client.submit_package_report(&extra_packages).await.unwrap();

    // should only queue one of the packages, since they're friends
    client
        .request_rebuild(QueueJobRequest {
            distribution: None,
            release: None,
            component: None,
            name: None,
            version: None,
            architecture: None,
            status: None,
            priority: Some(Priority::manual()),
        })
        .await
        .unwrap();

    let jobs = client
        .get_queued_jobs(None, None, None)
        .await
        .unwrap()
        .records;

    assert_eq!(1, jobs.len());

    isolated_server.shutdown().await;
}

#[rstest]
#[tokio::test]
pub async fn can_requeue_package_beyond_max_retries(
    #[with(None, Some(1), None)] config_file: ConfigFile,
    #[with(config_file.clone())] mut isolated_server: IsolatedServer,
) {
    let client = &isolated_server.client;
    let _config_file = config_file;

    setup_build_ready_database(client).await;

    // first attempt, get and report
    report_bad_rebuild(client).await;

    // ensure package is not enqueued after failed attempt
    let jobs = client
        .get_queued_jobs(None, None, None)
        .await
        .unwrap()
        .records;

    assert!(jobs.is_empty());

    // request a requeue
    client
        .request_rebuild(QueueJobRequest {
            distribution: None,
            release: None,
            component: None,
            name: None,
            version: None,
            architecture: None,
            status: None,
            priority: Some(Priority::manual()),
        })
        .await
        .unwrap();

    let jobs = client
        .get_queued_jobs(None, None, None)
        .await
        .unwrap()
        .records;

    assert_eq!(1, jobs.len());

    isolated_server.shutdown().await;
}
