use crate::actions::*;
use crate::data::*;
use crate::database::*;
use crate::fixtures::server::IsolatedServer;
use crate::fixtures::*;
use crate::setup;
use rand::distr::{Alphanumeric, SampleString};
use rebuilderd_common::api::v1::{QueueRestApi, WorkerRestApi};
use rebuilderd_common::config::PING_DEADLINE;
use rstest::rstest;

#[rstest]
#[tokio::test]
pub async fn returns_no_results_for_empty_database(mut isolated_server: IsolatedServer) {
    let client = &isolated_server.client;

    let results = client.get_workers(None).await.unwrap().records;

    assert!(results.is_empty());

    isolated_server.shutdown().await;
}

#[rstest]
#[tokio::test]
pub async fn returns_single_result_for_database_with_single_worker(
    mut isolated_server: IsolatedServer,
) {
    let client = &isolated_server.client;

    register_worker(client).await;

    let results = client.get_workers(None).await.unwrap().records;

    assert_eq!(1, results.len());

    isolated_server.shutdown().await;
}

#[rstest]
#[tokio::test]
pub async fn returns_multiple_results_for_database_with_multiple_workers(
    mut isolated_server: IsolatedServer,
) {
    let client = &mut isolated_server.client;

    register_worker(client).await;

    // create a new key for the new worker
    let worker_key = Alphanumeric.sample_string(&mut rand::rng(), 32);
    client.worker_key(worker_key);
    register_other_worker(client).await;

    let results = client.get_workers(None).await.unwrap().records;

    assert_eq!(2, results.len());

    isolated_server.shutdown().await;
}

#[rstest]
#[tokio::test]
pub async fn does_not_need_authentication(mut isolated_server: IsolatedServer) {
    let client = &mut isolated_server.client;

    register_worker(client).await;

    // zero out keys
    client.auth_cookie("");
    client.worker_key("");
    client.signup_secret("");

    let result = client.get_workers(None).await;

    assert!(result.is_ok());

    isolated_server.shutdown().await;
}

#[rstest]
#[tokio::test]
pub async fn reports_recently_seen_worker_as_online(mut isolated_server: IsolatedServer) {
    let client = &isolated_server.client;

    register_worker(client).await;

    let results = client.get_workers(None).await.unwrap().records;

    assert!(results[0].is_online);

    isolated_server.shutdown().await;
}

#[rstest]
#[tokio::test]
pub async fn reports_worker_past_offline_deadline_as_offline(mut isolated_server: IsolatedServer) {
    let client = &isolated_server.client;

    setup::build_ready_database(client).await;
    pick_up_job(client).await;
    set_worker_last_ping(&isolated_server, seconds_ago(PING_DEADLINE + 60));

    let results = client.get_workers(None).await.unwrap().records;

    assert!(!results[0].is_online);
    assert_eq!(None, results[0].status);

    isolated_server.shutdown().await;
}

#[rstest]
#[tokio::test]
pub async fn reports_worker_within_configured_offline_deadline_as_online(
    #[with(config_file::partial_4(None, None, None, Some(PING_DEADLINE as u32 * 2)))]
    mut isolated_server: IsolatedServer,
) {
    let client = &isolated_server.client;

    register_worker(client).await;
    set_worker_last_ping(&isolated_server, seconds_ago(PING_DEADLINE + 60));

    let results = client.get_workers(None).await.unwrap().records;

    assert!(results[0].is_online);

    isolated_server.shutdown().await;
}

#[rstest]
#[tokio::test]
pub async fn reports_offline_worker_as_online_after_it_makes_contact(
    mut isolated_server: IsolatedServer,
) {
    let client = &isolated_server.client;

    register_worker(client).await;
    set_worker_last_ping(&isolated_server, seconds_ago(PING_DEADLINE + 60));

    let results = client.get_workers(None).await.unwrap().records;
    assert!(!results[0].is_online);

    client.request_work(job_request()).await.unwrap();

    let results = client.get_workers(None).await.unwrap().records;
    assert!(results[0].is_online);

    isolated_server.shutdown().await;
}
