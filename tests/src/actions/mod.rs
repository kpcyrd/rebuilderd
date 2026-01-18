use crate::data::*;
use rebuilderd_common::api::Client;
use rebuilderd_common::api::v1::{
    BuildRestApi, BuildStatus, JobAssignment, PackageRestApi, PopQueuedJobRequest, Priority,
    QueueJobRequest, QueueRestApi, QueuedJobWithArtifacts, RegisterWorkerRequest, WorkerRestApi,
};

pub async fn register_worker(client: &Client) {
    client
        .register_worker(RegisterWorkerRequest {
            name: DUMMY_WORKER.to_string(),
        })
        .await
        .unwrap();
}

pub async fn register_other_worker(client: &Client) {
    client
        .register_worker(RegisterWorkerRequest {
            name: DUMMY_OTHER_WORKER.to_string(),
        })
        .await
        .unwrap();
}

pub async fn import_single_package(client: &Client) {
    client
        .submit_package_report(&single_package_report())
        .await
        .unwrap();
}

pub async fn import_single_package_with_multiple_artifacts(client: &Client) {
    client
        .submit_package_report(&single_package_with_multiple_artifacts_report())
        .await
        .unwrap();
}

pub async fn import_multiple_packages(client: &Client) {
    client
        .submit_package_report(&multiple_package_report())
        .await
        .unwrap();
}

pub async fn pick_up_job(client: &Client) -> QueuedJobWithArtifacts {
    match client
        .request_work(PopQueuedJobRequest {
            supported_backends: vec![
                DUMMY_BACKEND.to_string(),
                DUMMY_OTHER_DISTRIBUTION.to_string(),
            ],
            architecture: DUMMY_ARCHITECTURE.to_string(),
            supported_architectures: vec![
                DUMMY_ARCHITECTURE.to_string(),
                DUMMY_OTHER_ARCHITECTURE.to_string(),
            ],
        })
        .await
        .unwrap()
    {
        JobAssignment::Rebuild(item) => *item,
        _ => panic!("Expected a job assignment"),
    }
}

pub async fn report_bad_rebuild(client: &Client) {
    let job = pick_up_job(client).await;
    let report = bad_rebuild_report(&job);

    client.submit_build_report(report).await.unwrap();
}

pub async fn report_failed_rebuild(client: &Client) {
    let job = pick_up_job(client).await;
    let report = failed_rebuild_report(&job);

    client.submit_build_report(report).await.unwrap();
}

pub async fn report_good_rebuild(client: &Client) {
    let job = pick_up_job(client).await;
    let report = good_rebuild_report(&job);

    client.submit_build_report(report).await.unwrap();
}

pub async fn report_good_rebuild_with_signed_attestation(client: &Client) {
    let job = pick_up_job(client).await;
    let report = good_rebuild_report_with_signed_attestation(&job).await;

    client.submit_build_report(report).await.unwrap();
}

pub async fn report_good_rebuild_with_unsigned_attestation(client: &Client) {
    let job = pick_up_job(client).await;
    let report = good_rebuild_report_with_unsigned_attestation(&job).await;

    client.submit_build_report(report).await.unwrap();
}

pub async fn request_rebuild_of_all_bad_packages(client: &Client) {
    client
        .request_rebuild(QueueJobRequest {
            distribution: None,
            release: None,
            component: None,
            name: None,
            version: None,
            architecture: None,
            status: Some(BuildStatus::Bad),
            priority: None,
        })
        .await
        .unwrap();
}
