use crate::data::{
    DUMMY_ARCHITECTURE, DUMMY_BACKEND, DUMMY_WORKER, create_dummy_signed_attestation,
    create_dummy_unsigned_attestation, single_package_report,
    single_package_with_multiple_artifacts_report,
};
use chrono::Utc;
use rebuilderd_common::api::Client;
use rebuilderd_common::api::v1::{
    ArtifactStatus, BuildRestApi, BuildStatus, JobAssignment, PackageRestApi, PopQueuedJobRequest,
    Priority, QueueJobRequest, QueueRestApi, RebuildArtifactReport, RebuildReport,
    RegisterWorkerRequest, WorkerRestApi,
};
use rebuilderd_common::utils::zstd_compress;

pub async fn register_worker(client: &Client) {
    client
        .register_worker(RegisterWorkerRequest {
            name: DUMMY_WORKER.to_string(),
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

pub async fn report_bad_rebuild_for_single_package(client: &Client) {
    let job = match client
        .request_work(PopQueuedJobRequest {
            supported_backends: vec![DUMMY_BACKEND.to_string()],
            architecture: DUMMY_ARCHITECTURE.to_string(),
            supported_architectures: vec![DUMMY_ARCHITECTURE.to_string()],
        })
        .await
        .unwrap()
    {
        JobAssignment::Rebuild(item) => *item,
        _ => panic!("Expected a job assignment"),
    };

    let mut artifacts = Vec::new();
    for artifact in job.artifacts.clone() {
        artifacts.push(RebuildArtifactReport {
            name: artifact.name.clone(),
            diffoscope: None,
            status: ArtifactStatus::Bad,
            attestation: None,
        });
    }

    let report = RebuildReport {
        queue_id: job.job.id,
        built_at: Utc::now().naive_utc(),
        build_log: String::new().into_bytes(),
        status: BuildStatus::Bad,
        artifacts,
    };

    client.submit_build_report(report).await.unwrap();
}

pub async fn report_good_rebuild_for_single_package(client: &Client) {
    let job = match client
        .request_work(PopQueuedJobRequest {
            supported_backends: vec![DUMMY_BACKEND.to_string()],
            architecture: DUMMY_ARCHITECTURE.to_string(),
            supported_architectures: vec![DUMMY_ARCHITECTURE.to_string()],
        })
        .await
        .unwrap()
    {
        JobAssignment::Rebuild(item) => *item,
        _ => panic!("Expected a job assignment"),
    };

    let mut artifacts = Vec::new();
    for artifact in job.artifacts.clone() {
        artifacts.push(RebuildArtifactReport {
            name: artifact.name.clone(),
            diffoscope: None,
            status: ArtifactStatus::Good,
            attestation: None,
        });
    }

    let report = RebuildReport {
        queue_id: job.job.id,
        built_at: Utc::now().naive_utc(),
        build_log: String::new().into_bytes(),
        status: BuildStatus::Good,
        artifacts,
    };

    client.submit_build_report(report).await.unwrap();
}

pub async fn report_good_rebuild_with_signed_attestation_for_single_package(client: &Client) {
    let job = match client
        .request_work(PopQueuedJobRequest {
            supported_backends: vec![DUMMY_BACKEND.to_string()],
            architecture: DUMMY_ARCHITECTURE.to_string(),
            supported_architectures: vec![DUMMY_ARCHITECTURE.to_string()],
        })
        .await
        .unwrap()
    {
        JobAssignment::Rebuild(item) => *item,
        _ => panic!("Expected a job assignment"),
    };

    let input = job.job.url.rsplit_once("/").unwrap().1;

    let mut artifacts = Vec::new();
    for artifact in job.artifacts.clone() {
        let attestation = create_dummy_signed_attestation(input, &artifact.name);

        artifacts.push(RebuildArtifactReport {
            name: artifact.name.clone(),
            diffoscope: None,
            status: ArtifactStatus::Good,
            attestation: Some(zstd_compress(attestation.as_bytes()).await.unwrap()),
        });
    }

    let report = RebuildReport {
        queue_id: job.job.id,
        built_at: Utc::now().naive_utc(),
        build_log: String::new().into_bytes(),
        status: BuildStatus::Good,
        artifacts,
    };

    client.submit_build_report(report).await.unwrap();
}

pub async fn report_good_rebuild_with_unsigned_attestation_for_single_package(client: &Client) {
    let job = match client
        .request_work(PopQueuedJobRequest {
            supported_backends: vec![DUMMY_BACKEND.to_string()],
            architecture: DUMMY_ARCHITECTURE.to_string(),
            supported_architectures: vec![DUMMY_ARCHITECTURE.to_string()],
        })
        .await
        .unwrap()
    {
        JobAssignment::Rebuild(item) => *item,
        _ => panic!("Expected a job assignment"),
    };

    let input = job.job.url.rsplit_once("/").unwrap().1;

    let mut artifacts = Vec::new();
    for artifact in job.artifacts.clone() {
        let attestation = create_dummy_unsigned_attestation(input, &artifact.name);

        artifacts.push(RebuildArtifactReport {
            name: artifact.name.clone(),
            diffoscope: None,
            status: ArtifactStatus::Good,
            attestation: Some(zstd_compress(attestation.as_bytes()).await.unwrap()),
        });
    }

    let report = RebuildReport {
        queue_id: job.job.id,
        built_at: Utc::now().naive_utc(),
        build_log: String::new().into_bytes(),
        status: BuildStatus::Good,
        artifacts,
    };

    client.submit_build_report(report).await.unwrap();
}

pub async fn request_rebuild_of_single_package(client: &Client) {
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
