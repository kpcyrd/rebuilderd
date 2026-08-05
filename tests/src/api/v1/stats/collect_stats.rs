use crate::data::*;
use crate::fixtures::server::IsolatedServer;
use crate::fixtures::*;
use crate::setup;
use rebuilderd_common::api::v1::{StatsCollectRequest, StatsRestApi};
use rstest::rstest;

#[rstest]
#[tokio::test]
pub async fn returns_collected_snapshot(mut isolated_server: IsolatedServer) {
    let client = &isolated_server.client;

    setup::single_good_rebuild(client).await;

    // Pass explicit distribution to trigger single-combo collection mode
    // (the no-filter path enumerates configured backends, which are empty in tests)
    let snapshots = client
        .collect_stats(StatsCollectRequest {
            backend: None,
            distribution: Some(DUMMY_DISTRIBUTION.to_string()),
            release: None,
            architecture: None,
        })
        .await
        .unwrap();

    assert!(!snapshots.is_empty());

    isolated_server.shutdown().await;
}

#[rstest]
#[tokio::test]
pub async fn snapshot_counts_reflect_rebuild_status(mut isolated_server: IsolatedServer) {
    let client = &isolated_server.client;

    setup::single_good_rebuild(client).await;

    let snapshots = client
        .collect_stats(StatsCollectRequest {
            backend: None,
            distribution: Some(DUMMY_DISTRIBUTION.to_string()),
            release: None,
            architecture: None,
        })
        .await
        .unwrap();

    let total_good: i32 = snapshots.iter().map(|s| s.good).sum();
    assert_eq!(1, total_good);

    isolated_server.shutdown().await;
}

#[rstest]
#[tokio::test]
pub async fn requires_authentication(mut isolated_server: IsolatedServer) {
    let client = &mut isolated_server.client;

    // zero out admin key
    client.auth_cookie("");

    let result = client
        .collect_stats(StatsCollectRequest {
            backend: None,
            distribution: None,
            release: None,
            architecture: None,
        })
        .await;

    assert!(result.is_err());

    isolated_server.shutdown().await;
}
