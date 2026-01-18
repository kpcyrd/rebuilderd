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
