use crate::actions::*;
use rebuilderd_common::api::Client;

pub async fn registered_worker(client: &Client) {
    register_worker(client).await;
}

pub async fn single_imported_package(client: &Client) {
    import_single_package(client).await;
}

pub async fn single_imported_package_with_multiple_artifacts(client: &Client) {
    import_single_package_with_multiple_artifacts(client).await;
}

pub async fn multiple_imported_packages(client: &Client) {
    import_multiple_packages(client).await;
}

pub async fn single_imported_package_with_null_release(client: &Client) {
    import_single_package_with_null_release(client).await;
}

pub async fn single_bad_rebuild(client: &Client) {
    register_worker(client).await;
    import_single_package(client).await;
    report_bad_rebuild(client).await;
}

pub async fn single_good_rebuild(client: &Client) {
    register_worker(client).await;
    import_single_package(client).await;
    report_good_rebuild(client).await;
}

pub async fn single_failed_rebuild(client: &Client) {
    register_worker(client).await;
    import_single_package(client).await;
    report_failed_rebuild(client).await;
}

pub async fn single_rebuild_in_progress(client: &Client) {
    register_worker(client).await;
    import_single_package(client).await;
    pick_up_job(client).await;
}

pub async fn single_good_rebuild_with_signed_attestation(client: &Client) {
    register_worker(client).await;
    import_single_package(client).await;
    report_good_rebuild_with_signed_attestation(client).await;
}

pub async fn single_good_rebuild_with_unsigned_attestation(client: &Client) {
    register_worker(client).await;
    import_single_package(client).await;
    report_good_rebuild_with_unsigned_attestation(client).await;
}

pub async fn single_rebuild_request(client: &Client) {
    register_worker(client).await;
    import_single_package(client).await;
    report_bad_rebuild(client).await;
    request_rebuild_of_all_bad_packages(client).await;
}

pub async fn build_ready_database(client: &Client) {
    register_worker(client).await;
    import_single_package(client).await;
}
