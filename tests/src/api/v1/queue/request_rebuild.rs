use crate::actions::*;
use crate::data::*;
use crate::fixtures::server::IsolatedServer;
use crate::fixtures::*;
use crate::setup::setup_single_rebuild_request;
use chrono::Utc;
use rebuilderd_common::api::v1::{
    BuildStatus, IdentityFilter, OriginFilter, PackageReport, PackageRestApi, Priority,
    QueueJobRequest, QueueRestApi,
};
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
pub async fn requeued_packages_are_queued_with_manual_priority_by_default(
    isolated_server: IsolatedServer,
) {
    let client = isolated_server.client;

    setup_single_rebuild_request(&client).await;

    let job = client
        .get_queued_jobs(None, None, None)
        .await
        .unwrap()
        .records
        .pop()
        .unwrap();

    assert_eq!(Priority::manual(), job.priority)
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

#[rstest]
#[tokio::test]
pub async fn fails_if_no_admin_authentication_is_provided(isolated_server: IsolatedServer) {
    let mut client = isolated_server.client;

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
}

#[rstest]
#[case(OriginFilter {
        distribution: None,
        release: Some(DUMMY_OTHER_RELEASE.to_string()),
        component: None,
        architecture: None,
    }, single_package_report_from_different_release())]
#[case(OriginFilter {
        distribution: None,
        release: None,
        component: Some(DUMMY_OTHER_COMPONENT.to_string()),
        architecture: None,
    }, single_package_report_from_different_component())]
#[tokio::test]
pub async fn does_not_requeue_friends(
    isolated_server: IsolatedServer,
    #[case] origin_filter: OriginFilter,
    #[case] extra_packages: PackageReport,
) {
    let client = isolated_server.client;

    // first, a single package with a bad rebuild
    setup_single_bad_rebuild(&client).await;

    // then, the friend of that package
    let friend_identity = IdentityFilter {
        name: Some(DUMMY_BINARY_PACKAGE.to_string()),
        version: Some(DUMMY_BINARY_PACKAGE_VERSION.to_string()),
    };

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
}
