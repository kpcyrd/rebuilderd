use crate::data::*;
use crate::fixtures::server::IsolatedServer;
use crate::fixtures::*;
use rebuilderd_common::api::v1::{RegisterWorkerRequest, WorkerRestApi};
use rstest::rstest;

#[rstest]
#[tokio::test]
pub async fn worker_can_sign_up(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    client
        .register_worker(RegisterWorkerRequest {
            name: DUMMY_WORKER.to_string(),
        })
        .await
        .unwrap();
}

#[rstest]
#[tokio::test]
pub async fn fails_if_no_signup_authentication_is_provided(isolated_server: IsolatedServer) {
    let mut client = isolated_server.client;

    // zero out key
    client.signup_secret("");
    let result = client
        .register_worker(RegisterWorkerRequest {
            name: DUMMY_WORKER.to_string(),
        })
        .await;

    assert!(result.is_err());
}
