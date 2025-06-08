use crate::diesel::ExpressionMethods;
use crate::models::{NewBinaryPackage, NewBuildInput, NewQueued, NewSourcePackage};
use crate::schema::{build_inputs, rebuilds, source_packages};
use chrono::{DateTime, Utc};
use diesel::{Connection, OptionalExtension, QueryDsl};
use diesel::{RunQueryDsl, SqliteConnection};
use rebuilderd_common::api::v0::SuiteImport;
use rebuilderd_common::errors::*;

const DEFAULT_QUEUE_PRIORITY: i32 = 1;

fn sync(import: &SuiteImport, connection: &mut SqliteConnection) -> Result<()> {
    connection.transaction::<(), _, _>(|connection| {
        info!(
            "received submitted artifact groups {:?}",
            import.groups.len()
        );

        let artifact_count: usize = import.groups.iter().map(|g| g.artifacts.len()).sum();

        let mut progress_group_insert = 0;
        let mut progress_pkg_inserts = 0;

        for group_batch in import.groups.chunks(1_000) {
            progress_group_insert += group_batch.len();
            info!(
                "inserting new groups in batch: {}/{}",
                progress_group_insert,
                import.groups.len()
            );

            for group in group_batch {
                if log::log_enabled!(log::Level::Trace) {
                    trace!("group in this batch: {:?}", group);
                }

                let source_package_to_import = NewSourcePackage {
                    name: group.name.clone(),
                    version: group.version.clone(),
                    distribution: group.distro.clone(),
                    release: None,
                    component: Some(group.suite.clone()),
                };

                let source_package = source_package_to_import.upsert(connection)?;

                let build_input_to_import = NewBuildInput {
                    source_package_id: source_package.id,
                    url: group.input_url.clone().unwrap_or_default(), // TODO: behaviour change: is this guaranteed to exist?
                    backend: group.distro.clone(),
                    architecture: group.architecture.clone(),
                    retries: 0, // only used for new entries, old ones are kept by upsert
                };

                let build_input = build_input_to_import.upsert(connection)?;

                for artifact in &group.artifacts {
                    progress_pkg_inserts += 1;
                    info!(
                        "inserting new packages in batch: {}/{}",
                        progress_pkg_inserts, artifact_count
                    );
                    if log::log_enabled!(log::Level::Trace) {
                        trace!("pkg in this batch: {:?}", artifact);
                    }

                    let binary_package_to_import = NewBinaryPackage {
                        source_package_id: source_package.id,
                        build_input_id: build_input.id,
                        name: artifact.name.clone(),
                        version: artifact.version.clone(),
                        architecture: group.architecture.clone(), // TODO: behaviour change: source packages generating multiple architectures
                        artifact_url: artifact.url.clone(),
                    };

                    binary_package_to_import.upsert(connection)?;
                }

                let needs_rebuild = match rebuilds::table
                    .filter(rebuilds::build_input_id.eq(build_input.id))
                    .select(rebuilds::status)
                    .order_by(rebuilds::built_at.desc())
                    .get_result::<Option<String>>(connection)
                    .optional()?
                    .flatten()
                {
                    None => true,
                    Some(value) => value != "GOOD",
                };

                if needs_rebuild {
                    let now: DateTime<Utc> = Utc::now();
                    let queued_to_import = NewQueued {
                        build_input_id: build_input.id,
                        priority: DEFAULT_QUEUE_PRIORITY,
                        queued_at: now.naive_utc(),
                    };

                    let queued = queued_to_import.upsert(connection)?;

                    if log::log_enabled!(log::Level::Trace) {
                        trace!("queued in this batch: {:?}", queued);
                    }
                }
            }
        }

        Ok::<(), Error>(())
    })?;

    info!("successfully synced import to database");

    Ok(())
}

fn retry(import: &SuiteImport, connection: &mut SqliteConnection) -> Result<()> {
    info!("selecting packages with due retries");

    let queue = build_inputs::table
        .inner_join(source_packages::table)
        .filter(source_packages::distribution.eq(&import.distro))
        .filter(source_packages::component.eq(&import.suite))
        .select(build_inputs::id)
        .load::<i32>(connection)?;

    info!("queueing new retries");
    for build_input_ids in queue.chunks(1_000) {
        debug!("queue: {:?}", build_input_ids.len());

        let queue_items = build_input_ids
            .iter()
            .map(|id| NewQueued::new(*id, 2))
            .collect::<Vec<_>>();

        for queue_item in queue_items {
            queue_item.upsert(connection)?;
        }
    }
    info!("successfully triggered {} retries", queue.len());

    Ok(())
}

pub fn run(import: SuiteImport, connection: &mut SqliteConnection) -> Result<()> {
    sync(&import, connection)?;
    retry(&import, connection)?;

    Ok(())
}
