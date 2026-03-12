use crate::data::*;
use crate::fixtures::server::IsolatedServer;
use crate::fixtures::*;
use crate::setup;
use rebuilderd_common::api::v1::{StatsCollectRequest, StatsFilter, StatsRestApi};
use rstest::rstest;

#[rstest]
#[tokio::test]
pub async fn returns_empty_for_new_database(mut isolated_server: IsolatedServer) {
    let results = isolated_server
        .client
        .get_stats(None)
        .await
        .unwrap();

    assert!(results.is_empty());

    isolated_server.shutdown().await;
}

#[rstest]
#[tokio::test]
pub async fn returns_snapshot_after_collection(mut isolated_server: IsolatedServer) {
    let client = &isolated_server.client;

    setup::single_good_rebuild(client).await;

    client
        .collect_stats(StatsCollectRequest {
            backend: None,
            distribution: Some(DUMMY_DISTRIBUTION.to_string()),
            release: None,
            architecture: None,
        })
        .await
        .unwrap();

    let results = isolated_server.client.get_stats(None).await.unwrap();

    assert_eq!(1, results.len());

    isolated_server.shutdown().await;
}

#[rstest]
#[tokio::test]
pub async fn does_not_need_authentication(mut isolated_server: IsolatedServer) {
    let client = &mut isolated_server.client;

    // zero out keys
    client.auth_cookie("");
    client.worker_key("");
    client.signup_secret("");

    let result = client.get_stats(None).await;

    assert!(result.is_ok());

    isolated_server.shutdown().await;
}

#[rstest]
#[tokio::test]
pub async fn filter_by_distribution_returns_matching_snapshots(
    mut isolated_server: IsolatedServer,
) {
    let client = &isolated_server.client;

    setup::single_good_rebuild(client).await;

    client
        .collect_stats(StatsCollectRequest {
            backend: None,
            distribution: Some(DUMMY_DISTRIBUTION.to_string()),
            release: None,
            architecture: None,
        })
        .await
        .unwrap();

    let filter = StatsFilter {
        distribution: Some("nonexistent-distro".to_string()),
        release: None,
        architecture: None,
        since: None,
        limit: None,
    };

    let results = isolated_server
        .client
        .get_stats(Some(&filter))
        .await
        .unwrap();

    assert!(results.is_empty());

    isolated_server.shutdown().await;
}
