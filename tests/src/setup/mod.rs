use crate::actions::*;
use rebuilderd_common::api::Client;

pub async fn setup_registered_worker(client: &Client) {
    register_worker(client).await;
}

pub async fn setup_single_imported_package(client: &Client) {
    import_single_package(client).await;
}

pub async fn setup_single_imported_package_with_multiple_artifacts(client: &Client) {
    import_single_package_with_multiple_artifacts(&client).await;
}

pub async fn setup_multiple_imported_packages(client: &Client) {
    import_single_package(client).await;
    import_single_package_with_multiple_artifacts(client).await;
}

pub async fn setup_single_bad_rebuild(client: &Client) {
    register_worker(client).await;
    import_single_package(client).await;
    report_bad_rebuild_for_single_package(client).await;
}

pub async fn setup_single_good_rebuild(client: &Client) {
    register_worker(client).await;
    import_single_package(client).await;
    report_good_rebuild_for_single_package(client).await;
}

pub async fn setup_single_failed_rebuild(client: &Client) {
    register_worker(client).await;
    import_single_package(client).await;
    report_failed_rebuild_for_single_package(client).await;
}

pub async fn setup_single_rebuild_in_progress(client: &Client) {
    register_worker(client).await;
    import_single_package(client).await;
    pick_up_job(client).await;
}

pub async fn setup_single_good_rebuild_with_signed_attestation(client: &Client) {
    register_worker(client).await;
    import_single_package(client).await;
    report_good_rebuild_with_signed_attestation_for_single_package(client).await;
}

pub async fn setup_single_good_rebuild_with_unsigned_attestation(client: &Client) {
    register_worker(client).await;
    import_single_package(client).await;
    report_good_rebuild_with_unsigned_attestation_for_single_package(client).await;
}

pub async fn setup_single_rebuild_request(client: &Client) {
    register_worker(client).await;
    import_single_package(client).await;
    report_bad_rebuild_for_single_package(client).await;
    request_rebuild_of_single_package(client).await;
}

pub async fn setup_build_ready_database(client: &Client) {
    register_worker(client).await;
    import_single_package(client).await;
}
