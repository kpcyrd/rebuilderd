use crate::actions::*;
use crate::fixtures::server::IsolatedServer;
use crate::fixtures::*;
use crate::setup::*;
use rebuilderd::attestation;
use rebuilderd::attestation::Attestation;
use rebuilderd_common::api::v1::{BuildRestApi, MetaRestApi, PackageRestApi};
use rstest::rstest;

#[rstest]
#[tokio::test]
pub async fn returns_no_result_for_empty_database(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    let results = client.get_build_artifact_attestation(1, 1).await;

    assert!(results.is_err())
}

#[rstest]
#[tokio::test]
pub async fn returns_no_result_for_failed_build(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    setup_single_failed_rebuild(&client).await;

    let results = client.get_build_artifact_attestation(1, 1).await;

    assert!(results.is_err())
}

#[rstest]
#[tokio::test]
pub async fn returns_no_result_for_good_build_without_attestation(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    import_single_package(&client).await;
    register_worker(&client).await;
    report_good_rebuild(&client).await;

    let results = client.get_build_artifact_attestation(1, 1).await;

    assert!(results.is_err())
}

#[rstest]
#[tokio::test]
pub async fn artifact_has_attestation_for_good_build_with_signed_attestation(
    isolated_server: IsolatedServer,
) {
    let client = isolated_server.client;

    register_worker(&client).await;
    import_single_package(&client).await;
    report_good_rebuild_with_signed_attestation(&client).await;

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
pub async fn artifact_has_attestation_for_good_build_with_unsigned_attestation(
    isolated_server: IsolatedServer,
) {
    let client = isolated_server.client;

    register_worker(&client).await;
    import_single_package(&client).await;
    report_good_rebuild_with_unsigned_attestation(&client).await;

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

    register_worker(&client).await;
    import_single_package(&client).await;
    report_good_rebuild_with_signed_attestation(&client).await;

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

    register_worker(&client).await;
    import_single_package(&client).await;
    report_good_rebuild_with_unsigned_attestation(&client).await;

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
