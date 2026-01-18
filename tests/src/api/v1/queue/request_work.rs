use crate::actions::*;
use crate::data::*;
use crate::fixtures::server::IsolatedServer;
use crate::fixtures::*;
use crate::setup::*;
use rebuilderd_common::api::v1::{
    JobAssignment, PackageRestApi, PopQueuedJobRequest, QueueRestApi, RegisterWorkerRequest,
    WorkerRestApi,
};
use rstest::rstest;

#[rstest]
#[tokio::test]
pub async fn new_database_has_no_work(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    setup_registered_worker(&client).await;

    let job = client
        .request_work(PopQueuedJobRequest {
            supported_backends: vec![DUMMY_BACKEND.to_string()],
            architecture: DUMMY_ARCHITECTURE.to_string(),
            supported_architectures: vec![DUMMY_ARCHITECTURE.to_string()],
        })
        .await
        .unwrap();

    assert!(matches!(job, JobAssignment::Nothing))
}
#[rstest]
#[tokio::test]
pub async fn unregistered_worker_cannot_request_work(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    let result = client
        .request_work(PopQueuedJobRequest {
            supported_backends: vec![DUMMY_BACKEND.to_string()],
            architecture: DUMMY_ARCHITECTURE.to_string(),
            supported_architectures: vec![DUMMY_ARCHITECTURE.to_string()],
        })
        .await;

    assert!(result.is_err())
}

#[rstest]
#[tokio::test]
pub async fn registered_worker_can_request_work(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    register_worker(&client).await;
    import_single_package(&client).await;

    let job = client
        .request_work(PopQueuedJobRequest {
            supported_backends: vec![DUMMY_BACKEND.to_string()],
            architecture: DUMMY_ARCHITECTURE.to_string(),
            supported_architectures: vec![DUMMY_ARCHITECTURE.to_string()],
        })
        .await
        .unwrap();

    assert!(matches!(job, JobAssignment::Rebuild(_)))
}
