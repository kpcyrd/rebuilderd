use crate::actions::*;
use crate::data::DUMMY_SOURCE_PACKAGE;
use crate::fixtures::server::IsolatedServer;
use crate::fixtures::*;
use crate::setup::setup_single_rebuild_request;
use chrono::Utc;
use rebuilderd_common::api::v1::{BuildStatus, Priority, QueueJobRequest, QueueRestApi};
use rstest::rstest;

#[rstest]
#[tokio::test]
pub async fn can_requeue_bad_packages(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    register_worker(&client).await;
    import_single_package(&client).await;
    report_bad_rebuild(&client).await;

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
}

#[rstest]
#[tokio::test]
pub async fn requeued_packages_are_due_instantly(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    setup_single_rebuild_request(&client).await;

    let job = client
        .get_queued_jobs(None, None, None)
        .await
        .unwrap()
        .records
        .pop()
        .unwrap();

    assert!(job.next_retry.unwrap() <= Utc::now().naive_utc())
}

#[rstest]
#[tokio::test]
pub async fn requeued_packages_are_queued_with_default_priority(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    setup_single_rebuild_request(&client).await;

    let job = client
        .get_queued_jobs(None, None, None)
        .await
        .unwrap()
        .records
        .pop()
        .unwrap();

    assert_eq!(Priority::default(), job.priority)
}

#[rstest]
#[tokio::test]
pub async fn can_update_job_priority(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    setup_single_rebuild_request(&client).await;

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

    assert_eq!(Priority::manual(), job.priority)
}
