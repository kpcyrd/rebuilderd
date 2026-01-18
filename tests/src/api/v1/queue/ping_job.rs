use crate::actions::*;
use crate::fixtures::server::IsolatedServer;
use crate::fixtures::*;
use rebuilderd_common::api::v1::QueueRestApi;
use rstest::rstest;

#[rstest]
#[tokio::test]
pub async fn can_ping_running_job(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    register_worker(&client).await;
    import_single_package(&client).await;

    let job = pick_up_job(&client).await;

    client.ping_job(job.job.id).await.unwrap();
}
#[rstest]
#[tokio::test]
pub async fn can_not_ping_available_job(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    register_worker(&client).await;
    import_single_package(&client).await;

    let job = client.get_queued_job(1).await.unwrap();

    let result = client.ping_job(job.id).await;

    assert!(result.is_err());
}

#[rstest]
#[tokio::test]
pub async fn can_not_ping_nonexistent_job(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    register_worker(&client).await;
    import_single_package(&client).await;

    let result = client.ping_job(99999).await;

    assert!(result.is_err());
}
