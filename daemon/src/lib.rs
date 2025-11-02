use crate::config::Config;
use actix_web::middleware::{Logger, TrailingSlash};
use actix_web::web::{Data, JsonConfig, scope};
use actix_web::{App, HttpServer, middleware};
use in_toto::crypto::PrivateKey;
use rebuilderd_common::errors::*;
use std::sync::{Arc, RwLock};

pub mod api;
pub mod attestation;
pub mod code_migrations;
pub mod config;
pub mod db;
pub mod models;
pub mod schema;
pub mod web;

pub async fn run_config(pool: db::Pool, config: Config, privkey: PrivateKey) -> Result<()> {
    let bind_addr = config.bind_addr.clone();

    let privkey = Arc::new(privkey);

    HttpServer::new(move || {
        let json_config = JsonConfig::default().limit(config.post_body_size_limit);

        let v0_dashboard_cache = Arc::new(RwLock::new(api::v0::DashboardState::new()));

        App::new()
            .wrap(Logger::default())
            .wrap(middleware::Compress::default())
            .wrap(middleware::NormalizePath::new(TrailingSlash::Trim))
            .app_data(json_config)
            .app_data(Data::new(pool.clone()))
            .app_data(Data::new(config.clone()))
            .app_data(Data::new(privkey.clone()))
            .app_data(Data::new(v0_dashboard_cache.clone()))
            .service(
                scope("/api")
                    .service(
                        scope("/v0")
                            .service(api::v0::list_workers)
                            .service(api::v0::sync_work)
                            .service(api::v0::list_pkgs)
                            .service(api::v0::list_queue)
                            .service(api::v0::push_queue)
                            .service(api::v0::pop_queue)
                            .service(api::v0::drop_from_queue)
                            .service(api::v0::requeue_pkgbase)
                            .service(api::v0::ping_build)
                            .service(api::v0::report_build)
                            .service(api::v0::get_build_log)
                            .service(api::v0::get_attestation)
                            .service(api::v0::get_diffoscope)
                            .service(api::v0::get_dashboard)
                            .service(api::v0::get_public_key),
                    )
                    .service(
                        scope("/v1")
                            .service(
                                scope("/builds")
                                    .service(api::v1::get_builds)
                                    .service(api::v1::submit_rebuild_report)
                                    .service(api::v1::get_build)
                                    .service(api::v1::get_build_log)
                                    .service(api::v1::get_build_artifacts)
                                    .service(api::v1::get_build_artifact)
                                    .service(api::v1::get_build_artifact_diffoscope)
                                    .service(api::v1::get_build_artifact_attestation),
                            )
                            .service(scope("/dashboard").service(api::v1::get_dashboard))
                            .service(
                                scope("/meta")
                                    .service(api::v1::get_distributions)
                                    .service(api::v1::get_distribution_releases)
                                    .service(api::v1::get_distribution_architectures)
                                    .service(api::v1::get_distribution_components)
                                    .service(api::v1::get_distribution_release_architectures)
                                    .service(api::v1::get_distribution_release_components)
                                    .service(
                                        api::v1::get_distribution_release_component_architectures,
                                    )
                                    .service(api::v1::get_public_key),
                            )
                            .service(
                                scope("/packages")
                                    .service(api::v1::submit_package_report)
                                    .service(api::v1::get_source_packages)
                                    .service(api::v1::get_source_package)
                                    .service(api::v1::get_binary_packages)
                                    .service(api::v1::get_binary_package),
                            )
                            .service(
                                scope("/queue")
                                    .service(api::v1::get_queued_jobs)
                                    .service(api::v1::request_rebuild)
                                    .service(api::v1::get_queued_job)
                                    .service(api::v1::drop_queued_job)
                                    .service(api::v1::drop_queued_jobs)
                                    .service(api::v1::ping_job)
                                    .service(api::v1::request_work),
                            )
                            .service(
                                scope("/workers")
                                    .service(api::v1::get_workers)
                                    .service(api::v1::register_worker)
                                    .service(api::v1::get_worker)
                                    .service(api::v1::unregister_worker)
                                    .service(api::v1::get_worker_tags)
                                    .service(api::v1::set_worker_tags)
                                    .service(api::v1::create_worker_tag)
                                    .service(api::v1::delete_worker_tag),
                            )
                            .service(scope("/tags").service(api::v1::get_tags)),
                    ),
            )
    })
    .bind(&bind_addr)?
    .run()
    .await?;
    Ok(())
}
