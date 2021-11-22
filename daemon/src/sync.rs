use crate::models;
// use crate::versions::PkgVerCmp;
use diesel::SqliteConnection;
use rebuilderd_common::{PkgGroup, Status};
use rebuilderd_common::api::*;
use rebuilderd_common::errors::*;
// use std::cmp::Ordering;
use std::collections::HashMap;

const DEFAULT_QUEUE_PRIORITY: i32 = 1;

// this holds all pkgsbases we know about
// when syncing we remove all pkgbases that are still in the import
// the remaining pkgbases are not referenced in the new import that was submitted and groing to be deleted
pub struct CurrentArtifactNamespace {
    pkgbases: HashMap<String, models::PkgBase>,
}

impl CurrentArtifactNamespace {
    pub fn load_current_namespace_from_database(distro: &str, suite: &str, connection: &SqliteConnection) -> Result<Self> {
        let mut existing_pkgbases = HashMap::new();
        for pkgbase in models::PkgBase::list_distro_suite(distro, suite, connection)? {
            let key = Self::gen_key_for_pkgbase(&pkgbase);
            debug!("adding known pkgbase with key {:?} for distro={:?}, suite={:?}", key, distro, suite);
            trace!("adding known pkgbase with key {:?} for distro={:?}, suite={:?} to {:?}", key, distro, suite, pkgbase);
            existing_pkgbases.insert(key, pkgbase);
        }
        Ok(CurrentArtifactNamespace {
            pkgbases: existing_pkgbases,
        })
    }

    fn gen_key_for_pkgbase(pkgbase: &models::PkgBase) -> String {
        format!("{:?}-{:?}", pkgbase.name, pkgbase.version)
    }

    fn gen_key_for_pkggroup(pkggroup: &PkgGroup) -> String {
        format!("{:?}-{:?}", pkggroup.name, pkggroup.version)
    }

    // returns Some(()) if the group was already known and remove
    // returns None if the group wasn't known and nothing changed
    pub fn mark_pkggroup_still_present(&mut self, group: &PkgGroup) -> Option<()> {
        let key = Self::gen_key_for_pkggroup(group);
        if self.pkgbases.remove_entry(&key).is_some() {
            debug!("pkgbase is already present: {:?}", key);
            Some(())
        } else {
            debug!("pkgbase is not yet present: {:?}", key);
            None
        }
    }
}

// this holds all pkgbases from the import that we didn't know about yet
// all of them are going to be added to the database at the end
// builds also need to be scheduled afterwards
#[derive(Debug, Default)]
pub struct NewArtifactNamespace {
    groups: Vec<PkgGroup>,
}

impl NewArtifactNamespace {
    pub fn new() -> Self {
        NewArtifactNamespace::default()
    }

    pub fn add(&mut self, group: PkgGroup) {
        self.groups.push(group);
    }
}

// TODO: this should be into_api_item
fn pkggroup_to_newpkgbase(group: &PkgGroup) -> Result<models::NewPkgBase> {
    let artifacts = serde_json::to_string(&group.artifacts)?;
    Ok(models::NewPkgBase {
        name: group.name.clone(),
        version: group.version.clone(),
        distro: group.distro.clone(),
        suite: group.suite.clone(),
        architecture: group.architecture.clone(),
        input_url: group.input_url.clone(),
        artifacts,
        retries: 0,
        next_retry: None,
    })
}

fn sync(import: &SuiteImport, connection: &SqliteConnection) -> Result<()> {
    let distro = &import.distro;
    let suite = &import.suite;

    info!("received submitted artifact groups {:?}", import.groups.len());
    let mut new_namespace = NewArtifactNamespace::new();

    info!("loading existing artifact groups from database...");
    let mut current_namespace = CurrentArtifactNamespace::load_current_namespace_from_database(distro, suite, connection)?;
    info!("found existing artifact groups: len={}", current_namespace.pkgbases.len());

    info!("checking groups already in the database...");
    let mut num_already_in_database = 0;
    for group in &import.groups {
        trace!("received group during import: {:?}", group);
        if current_namespace.mark_pkggroup_still_present(group).is_some() {
            num_already_in_database += 1;
        } else {
            new_namespace.add(group.clone());
        }
    }
    info!("found groups already in database: len={}", num_already_in_database);
    info!("found groups that need to be added to database: len={}", new_namespace.groups.len());
    info!("found groups no longer present: len={}", current_namespace.pkgbases.len());

    for (key, pkgbase_to_remove) in current_namespace.pkgbases {
        debug!("deleting old group with key={:?}", key);
        models::PkgBase::delete(pkgbase_to_remove.id, connection)
            .with_context(|| anyhow!("Failed to delete pkgbase with key={:?}", key))?;
    }

    // inserting new groups
    let mut progress_group_insert = 0;
    for group_batch in new_namespace.groups.chunks(1_000) {
        progress_group_insert += group_batch.len();
        info!("inserting new groups in batch: {}/{}", progress_group_insert, new_namespace.groups.len());
        let group_batch = group_batch.iter()
            .map(pkggroup_to_newpkgbase)
            .collect::<Result<Vec<_>>>()?;
        if log::log_enabled!(log::Level::Trace) {
            for group in &group_batch {
                trace!("group in this batch: {:?}", group);
            }
        }
        models::NewPkgBase::insert_batch(&group_batch, connection)?;
    }

    // detecting pkgbase ids for new artifacts
    let mut progress_pkgbase_detect = 0;
    let mut backlog_insert_pkgs = Vec::new();
    let mut backlog_insert_queue = Vec::new();
    for group_batch in new_namespace.groups.chunks(1_000) {
        progress_pkgbase_detect += group_batch.len();
        info!("detecting pkgbase ids for new artifacts: {}/{}", progress_pkgbase_detect, new_namespace.groups.len());
        for group in group_batch {
            debug!("searching for pkgbases {:?} {:?} {:?} {:?} {:?}", group.name, group.version, distro, suite, group.architecture);
            let pkgbases = models::PkgBase::get_by(&group.name,
                                                  distro,
                                                  suite,
                                                  Some(&group.version),
                                                  Some(&group.architecture),
                                                  connection)?;

            if pkgbases.len() != 1 {
                bail!("Failed to determine pkgbase in database for grouop (expected=1, found={}): {:?}", pkgbases.len(), group);
            }
            let pkgbase_id = pkgbases[0].id;

            for artifact in &group.artifacts {
                backlog_insert_pkgs.push(models::NewPackage {
                    base_id: Some(pkgbase_id),
                    name: artifact.name.clone(),
                    version: artifact.version.clone(),
                    status: Status::Unknown.to_string(),
                    distro: distro.clone(),
                    suite: suite.clone(),
                    architecture: group.architecture.clone(),
                    artifact_url: artifact.url.clone(),
                    input_url: group.input_url.clone(), // TODO: this is deprecated
                    build_id: None,
                    built_at: None,
                    has_diffoscope: false,
                    has_attestation: false,
                    checksum: None,
                    retries: 0,
                    next_retry: None,
                });
            }

            backlog_insert_queue.push(models::NewQueued::new(pkgbase_id,
                                                             group.version.clone(),
                                                             distro.to_string(),
                                                             DEFAULT_QUEUE_PRIORITY));
        }
    }

    // inserting new packages
    let mut progress_pkg_inserts = 0;
    for pkg_batch in backlog_insert_pkgs.chunks(1_000) {
        progress_pkg_inserts += pkg_batch.len();
        info!("inserting new packages in batch: {}/{}", progress_pkg_inserts, backlog_insert_pkgs.len());
        if log::log_enabled!(log::Level::Trace) {
            for pkg in pkg_batch {
                trace!("pkg in this batch: {:?}", pkg);
            }
        }
        models::NewPackage::insert_batch(pkg_batch, connection)?;
    }

    // inserting to queue
    // TODO: check if queueing has been disabled in the request, eg. to initially fill the database
    let mut progress_queue_inserts = 0;
    for queue_batch in backlog_insert_queue.chunks(1_000) {
        progress_queue_inserts += queue_batch.len();
        info!("inserting to queue in batch: {}/{}", progress_queue_inserts, backlog_insert_queue.len());
        if log::log_enabled!(log::Level::Trace) {
            for queued in queue_batch {
                trace!("queued in this batch: {:?}", queued);
            }
        }
        models::Queued::insert_batch(queue_batch, connection)?;
    }

    info!("successfully synced import to database");

    Ok(())
}

fn retry(import: &SuiteImport, connection: &SqliteConnection) -> Result<()> {
    info!("selecting packages with due retries");
    let queue = models::PkgBase::list_distro_suite_due_retries(import.distro.as_ref(), &import.suite, connection)?;

    info!("queueing new retries");
    for bases in queue.chunks(1_000) {
        debug!("queue: {:?}", bases.len());
        models::Queued::queue_batch(bases, import.distro.to_string(), 2, connection)?;
    }
    info!("successfully triggered {} retries", queue.len());

    Ok(())
}

pub fn run(import: SuiteImport, connection: &SqliteConnection) -> Result<()> {
    sync(&import, connection)?;
    retry(&import, connection)?;
    Ok(())
}
