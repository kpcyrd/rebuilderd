use crate::actions::*;
use crate::fixtures::server::IsolatedServer;
use crate::fixtures::*;
use rebuilderd_common::api::v1::QueueRestApi;
use rstest::rstest;

#[rstest]
#[tokio::test]
pub async fn drops_job_correctly(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    import_single_package(&client).await;

    client.drop_queued_job(1).await.unwrap();

    let jobs = client
        .get_queued_jobs(None, None, None)
        .await
        .unwrap()
        .records;

    assert!(jobs.is_empty());
}

#[rstest]
#[tokio::test]
pub async fn fails_if_job_does_not_exist(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    import_single_package(&client).await;

    let result = client.drop_queued_job(9999).await;

    assert!(result.is_err());
}

#[rstest]
#[tokio::test]
pub async fn fails_if_no_admin_authentication_is_provided(isolated_server: IsolatedServer) {
    let mut client = isolated_server.client;

    import_multiple_packages(&client).await;

    // zero out key
    client.auth_cookie("");
    let result = client.drop_queued_job(1).await;

    assert!(result.is_err());
}
