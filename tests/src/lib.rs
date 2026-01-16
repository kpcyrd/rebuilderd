#![cfg(test)]

use crate::args::Args;
use crate::assertions::assert_job_matches_package;
use crate::data::{
    DUMMY_ARCHITECTURE, DUMMY_BACKEND, DUMMY_SOURCE_PACKAGE, single_package_report,
    single_package_with_multiple_artifacts_report,
};
use crate::fixtures::server::IsolatedServer;
use crate::fixtures::*;
use crate::setup::*;
use chrono::Utc;
use rebuilderd::attestation::{self, Attestation};
use rebuilderd_common::api::v1::{
    ArtifactStatus, BuildRestApi, JobAssignment, MetaRestApi, PackageRestApi, PopQueuedJobRequest,
    Priority, QueueJobRequest, QueueRestApi,
};
use rstest::rstest;

mod api;
mod args;
mod assertions;
mod data;
pub(crate) mod fixtures;
pub mod setup;

#[rstest]
#[tokio::test]
pub async fn worker_can_sign_up(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    setup_registered_worker(&client).await;
}

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
pub async fn can_import_multiple_times(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    setup_single_imported_package(&client).await;

    client
        .submit_package_report(&single_package_report())
        .await
        .unwrap();
}

#[rstest]
#[tokio::test]
pub async fn database_has_single_package_after_multiple_imports_of_same_package(
    isolated_server: IsolatedServer,
) {
    let client = isolated_server.client;

    setup_single_imported_package(&client).await;

    client
        .submit_package_report(&single_package_report())
        .await
        .unwrap();

    let packages = client
        .get_binary_packages(None, None, None)
        .await
        .map(|p| p.records)
        .unwrap();

    assert_eq!(packages.len(), 1)
}

#[rstest]
#[tokio::test]
pub async fn database_has_single_queued_job_after_single_package_import(
    isolated_server: IsolatedServer,
) {
    let client = isolated_server.client;

    setup_single_imported_package(&client).await;

    let jobs = client
        .get_queued_jobs(None, None, None)
        .await
        .unwrap()
        .records;

    assert_eq!(jobs.len(), 1)
}

#[rstest]
#[tokio::test]
pub async fn database_has_single_queued_job_after_multiple_imports_of_same_package(
    isolated_server: IsolatedServer,
) {
    let client = isolated_server.client;

    setup_single_imported_package(&client).await;

    client
        .submit_package_report(&single_package_report())
        .await
        .unwrap();

    let jobs = client
        .get_queued_jobs(None, None, None)
        .await
        .unwrap()
        .records;

    assert_eq!(jobs.len(), 1)
}

#[rstest]
#[tokio::test]
pub async fn queued_job_has_correct_data(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    let package_report = single_package_report();
    client.submit_package_report(&package_report).await.unwrap();

    let job = client
        .get_queued_jobs(None, None, None)
        .await
        .unwrap()
        .records
        .pop()
        .unwrap();

    let package = &package_report.packages[0];

    assert_job_matches_package(&package_report, package, &job);
    assert_eq!(None, job.next_retry);
    assert_eq!(Priority::default(), job.priority);
    assert_eq!(None, job.started_at);
    assert!(job.queued_at <= Utc::now().naive_utc());
}

#[rstest]
#[tokio::test]
pub async fn queued_job_has_correct_data_after_multiple_imports_of_same_package(
    isolated_server: IsolatedServer,
) {
    let client = isolated_server.client;

    setup_single_imported_package(&client).await;

    let package_report = single_package_report();
    client.submit_package_report(&package_report).await.unwrap();

    let job = client
        .get_queued_jobs(None, None, None)
        .await
        .unwrap()
        .records
        .pop()
        .unwrap();

    let package = &package_report.packages[0];

    assert_job_matches_package(&package_report, package, &job);
    assert_eq!(None, job.next_retry);
    assert_eq!(Priority::default(), job.priority);
    assert_eq!(None, job.started_at);
    assert!(job.queued_at <= Utc::now().naive_utc());
}

#[rstest]
#[tokio::test]
pub async fn registered_worker_can_request_work(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    setup_build_ready_database(&client).await;

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

#[rstest]
#[tokio::test]
pub async fn can_report_bad_rebuild(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    setup_single_bad_rebuild(&client).await;
}

#[rstest]
#[tokio::test]
pub async fn package_is_marked_bad_after_bad_report(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    setup_single_bad_rebuild(&client).await;

    let package = client
        .get_binary_packages(None, None, None)
        .await
        .map(|p| p.records)
        .unwrap()
        .pop()
        .unwrap();

    assert_eq!(Some(ArtifactStatus::Bad), package.status)
}

#[rstest]
#[tokio::test]
pub async fn package_is_requeued_after_bad_report(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    setup_single_bad_rebuild(&client).await;

    let package_report = single_package_report();
    let job = client
        .get_queued_jobs(None, None, None)
        .await
        .unwrap()
        .records
        .pop()
        .unwrap();

    let package = &package_report.packages[0];

    assert_job_matches_package(&package_report, package, &job);
    assert!(job.next_retry.unwrap() >= Utc::now().naive_utc());
    assert_eq!(Priority::retry(), job.priority);
    assert_eq!(None, job.started_at);
    assert!(job.queued_at <= Utc::now().naive_utc());
}

#[rstest]
#[tokio::test]
pub async fn can_requeue_bad_packages(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    setup_single_rebuild_request(&client).await;
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
            name: Some(DUMMY_SOURCE_PACKAGE.to_string()),
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
pub async fn can_report_good_rebuild(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    setup_single_good_rebuild(&client).await;
}

#[rstest]
#[tokio::test]
pub async fn package_is_marked_good_after_good_report(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    setup_single_good_rebuild(&client).await;

    let package = client
        .get_binary_packages(None, None, None)
        .await
        .map(|p| p.records)
        .unwrap()
        .pop()
        .unwrap();

    assert_eq!(Some(ArtifactStatus::Good), package.status)
}

#[rstest]
#[tokio::test]
pub async fn package_is_not_requeued_after_good_report(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    setup_single_good_rebuild(&client).await;

    let jobs = client
        .get_queued_jobs(None, None, None)
        .await
        .unwrap()
        .records;

    assert!(jobs.is_empty())
}

#[rstest]
#[tokio::test]
pub async fn can_import_single_package_with_multiple_artifacts(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    setup_single_imported_package_with_multiple_artifacts(&client).await;
}

#[rstest]
#[tokio::test]
pub async fn database_has_two_binary_packages_after_single_package_with_multiple_artifacts_import(
    isolated_server: IsolatedServer,
) {
    let client = isolated_server.client;

    setup_single_imported_package_with_multiple_artifacts(&client).await;

    let packages = client
        .get_binary_packages(None, None, None)
        .await
        .map(|p| p.records)
        .unwrap();

    assert_eq!(packages.len(), 2)
}

#[rstest]
#[tokio::test]
pub async fn database_has_single_queued_job_after_single_package_with_multiple_artifacts_import(
    isolated_server: IsolatedServer,
) {
    let client = isolated_server.client;

    setup_single_imported_package_with_multiple_artifacts(&client).await;

    let jobs = client
        .get_queued_jobs(None, None, None)
        .await
        .unwrap()
        .records;

    assert_eq!(jobs.len(), 1)
}

#[rstest]
#[tokio::test]
pub async fn single_package_multiple_artifatcs_queued_job_has_correct_data(
    isolated_server: IsolatedServer,
) {
    let client = isolated_server.client;

    let package_report = single_package_with_multiple_artifacts_report();
    client.submit_package_report(&package_report).await.unwrap();

    let job = client
        .get_queued_jobs(None, None, None)
        .await
        .unwrap()
        .records
        .pop()
        .unwrap();

    let package = &package_report.packages[0];

    assert_job_matches_package(&package_report, package, &job);
    assert_eq!(None, job.next_retry);
    assert_eq!(Priority::default(), job.priority);
    assert_eq!(None, job.started_at);
    assert!(job.queued_at <= Utc::now().naive_utc());
}

#[rstest]
#[tokio::test]
pub async fn can_report_good_rebuild_with_attestations(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    setup_single_good_rebuild_with_signed_attestation(&client).await;
}

#[rstest]
#[tokio::test]
pub async fn can_fetch_public_keys(isolated_server: IsolatedServer, program_arguments: Args) {
    let client = isolated_server.client;
    let public_key = isolated_server.public_key;

    let response = client.get_public_keys().await.unwrap();

    let mut keys = Vec::new();
    for pem in response.current {
        for key in attestation::pem_to_pubkeys(pem.as_bytes()).unwrap() {
            keys.push(key.unwrap());
        }
    }

    // if --no-daemon is set, this is expected to mismatch
    if !program_arguments.no_daemon && keys != [public_key.clone()] {
        assert_eq!([public_key], keys.as_slice());
    }
}

#[rstest]
#[tokio::test]
pub async fn artifact_has_attestation(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    setup_single_good_rebuild_with_signed_attestation(&client).await;

    let package = client
        .get_binary_packages(None, None, None)
        .await
        .map(|p| p.records)
        .unwrap()
        .pop()
        .unwrap();

    let build_id = package.build_id.unwrap();
    let artifact_id = package.artifact_id.unwrap();

    let artifact = client
        .get_build_artifact(build_id, artifact_id)
        .await
        .unwrap();

    assert!(artifact.has_attestation);

    client
        .get_build_artifact_attestation(build_id, artifact_id)
        .await
        .unwrap();
}

#[rstest]
#[tokio::test]
pub async fn attestation_is_signed_by_server(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    setup_single_good_rebuild_with_unsigned_attestation(&client).await;

    let package = client
        .get_binary_packages(None, None, None)
        .await
        .map(|p| p.records)
        .unwrap()
        .pop()
        .unwrap();

    let build_id = package.build_id.unwrap();
    let artifact_id = package.artifact_id.unwrap();

    let attestation = client
        .get_build_artifact_attestation(build_id, artifact_id)
        .await
        .unwrap();

    let attestation = Attestation::parse(&attestation).unwrap();

    let response = client.get_public_keys().await.unwrap();

    let mut keys = Vec::new();
    for pem in response.current {
        for key in attestation::pem_to_pubkeys(pem.as_bytes()).unwrap() {
            keys.push(key.unwrap());
        }
    }

    attestation.verify(1, &keys).unwrap();
}

#[rstest]
#[tokio::test]
pub async fn server_transparently_signs_attestations(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    setup_single_good_rebuild_with_unsigned_attestation(&client).await;

    let package = client
        .get_binary_packages(None, None, None)
        .await
        .map(|p| p.records)
        .unwrap()
        .pop()
        .unwrap();

    let build_id = package.build_id.unwrap();
    let artifact_id = package.artifact_id.unwrap();

    let attestation = client
        .get_build_artifact_attestation(build_id, artifact_id)
        .await
        .unwrap();

    let attestation = Attestation::parse(&attestation).unwrap();

    let response = client.get_public_keys().await.unwrap();

    let mut keys = Vec::new();
    for pem in response.current {
        for key in attestation::pem_to_pubkeys(pem.as_bytes()).unwrap() {
            keys.push(key.unwrap());
        }
    }

    attestation.verify(1, &keys).unwrap();
}
