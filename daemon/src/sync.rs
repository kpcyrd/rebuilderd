use crate::models;
use crate::versions::PkgVerCmp;
use diesel::SqliteConnection;
use rebuilderd_common::PkgRelease;
use rebuilderd_common::api::*;
use rebuilderd_common::errors::*;
use std::cmp::Ordering;
use std::collections::HashMap;

fn get_known_pkgbases(import: &mut SuiteImport, connection: &SqliteConnection) -> Result<HashMap<String, models::PkgBase>> {
    let mut known_pkgbases = HashMap::new();
    for pkgbase in models::PkgBase::list_distro_suite(import.distro.as_ref(), &import.suite, connection)? {
        debug!("known pkgbase: {}-{}", pkgbase.name, pkgbase.version);
        known_pkgbases.insert(format!("{}-{}", pkgbase.name, pkgbase.version), pkgbase);
    }
    Ok(known_pkgbases)
}

fn insert_pkgbases(import: &mut SuiteImport, connection: &SqliteConnection) -> Result<Vec<(String, PkgRelease)>> {
    // expand groups into individual packages
    let known_pkgbases = get_known_pkgbases(import, connection)?;
    let mut removed_pkgbases = known_pkgbases.clone();

    let mut import_pkgs = Vec::new();
    let mut insert_pkgbases = Vec::new();
    for mut base in import.pkgs.drain(..) {
        for artifact in base.artifacts.drain(..) {
            import_pkgs.push((base.base.clone(), PkgRelease::new(
                artifact.name,
                base.version.clone(),
                import.distro,
                base.suite.clone(),
                base.architecture.clone(),
                artifact.url,
            )));
        }
        let key = format!("{}-{}", base.base, base.version);
        removed_pkgbases.remove(&key);
        if !known_pkgbases.contains_key(&key) {
            debug!("adding pkgbase to insert queue: {:?}", base);
            insert_pkgbases.push(models::NewPkgBase {
                name: base.base,
                version: base.version,
                distro: base.distro,
                suite: base.suite,
                architecture: base.architecture,
                retries: 0,
                next_retry: None,
            });
        }
    }

    info!("inserting pkgbases ({})", insert_pkgbases.len());
    for bases in insert_pkgbases.chunks(1_000) {
        debug!("pkgbase: {:?}", bases.len());
        models::NewPkgBase::insert_batch(bases, connection)?;
    }

    // TODO: doing this could from the packages table, we need more refactoring first
    /*
    info!("removing pkgbases ({})", removed_pkgbases.len());
    for (_, v) in removed_pkgbases {
        models::PkgBase::delete(v.id, connection)?;
    }
    */

    Ok(import_pkgs)
}

fn sync(import: &mut SuiteImport, connection: &SqliteConnection) -> Result<()> {
    info!("submitted packages {:?}", import.pkgs.len());

    // expand groups into individual packages
    let mut import_pkgs = insert_pkgbases(import, connection)?;

    // ensure base_id is set
    // TODO: remove this after a few releases until we're sure base_id is always set
    for (base, pkg) in &import_pkgs {
        let existing_pkgs = models::Package::get_by(&pkg.name, &pkg.distro, &pkg.suite, None, connection)?;
        for mut existing in existing_pkgs {
            trace!("existing package: {:?}", existing);
            if existing.base_id.is_none() {
                debug!("fixing base_id on: {:?}", existing);
                let pkgbases = models::PkgBase::get_by(base, &pkg.distro, &pkg.suite, Some(&pkg.architecture), connection)?
                    .into_iter()
                    .filter(|b| b.version == pkg.version)
                    .collect::<Vec<_>>();

                if pkgbases.len() != 1 {
                    bail!("Failed to locate pkgbase: {:?}/{:?}/{:?} ({:?}, {:?})", base, pkg.distro, pkg.suite, pkg.version, pkg.architecture);
                }
                let pkgbase = &pkgbases[0];

                existing.base_id = Some(pkgbase.id);
                existing.update(connection)?;
            }
        }
    }

    // run regular import
    let pkgs = models::Package::list_distro_suite(import.distro.as_ref(), &import.suite, connection)?;

    let mut pkgs = pkgs.into_iter()
        .map(|pkg| (pkg.name.clone(), pkg))
        .collect::<HashMap<_, _>>();
    info!("existing packages {:?}", pkgs.len());

    let mut new_pkgs = HashMap::<_, (String, PkgRelease)>::new();
    let mut updated_pkgs = HashMap::<_, (String, models::Package)>::new();
    let mut deleted_pkgs = pkgs.clone();

    for (base, pkg) in import_pkgs.drain(..) {
        deleted_pkgs.remove_entry(&pkg.name);

        if let Some((_, cur)) = new_pkgs.get_mut(&pkg.name) {
            cur.bump_package(&import.distro, &pkg)?;
        } else if let Some((_, cur)) = updated_pkgs.get_mut(&pkg.name) {
            cur.bump_package(&import.distro, &pkg)?;
        } else if let Some(old) = pkgs.get_mut(&pkg.name) {
            if old.bump_package(&import.distro, &pkg)? == Ordering::Greater {
                updated_pkgs.insert(pkg.name, (base, old.clone()));
            }
        } else {
            new_pkgs.insert(pkg.name.clone(), (base, pkg));
        }
    }

    // TODO: consider starting a transaction here
    let mut queue = Vec::<(i32, String)>::new();

    // TODO: if the package is queued, don't queue it again. Right now we can't rebuild the non-latest version anyway

    info!("new packages: {:?}", new_pkgs.len());
    let mut insert_pkgs = Vec::new();
    for (_, (base, v)) in new_pkgs {
        let pkgbases = models::PkgBase::get_by(&base, &v.distro, &v.suite, Some(&v.architecture), connection)?
            .into_iter()
            .filter(|b| b.version == v.version)
            .collect::<Vec<_>>();

        if pkgbases.len() != 1 {
            bail!("Failed to locate pkgbase: {:?}/{:?}/{:?} ({:?}, {:?})", base, v.distro, v.suite, v.version, v.architecture);
        }
        let pkgbase = &pkgbases[0];

        insert_pkgs.push(models::NewPackage::from_api(import.distro, pkgbase.id, v));
    }

    for insert_pkgs in insert_pkgs.chunks(1_000) {
        debug!("new: {:?}", insert_pkgs.len());
        models::NewPackage::insert_batch(insert_pkgs, connection)?;

        // this is needed because diesel doesn't return ids when inserting into sqlite
        // this is obviously slow and needs to be refactored
        for new_pkg in insert_pkgs {
            let pkgs = models::Package::get_by(&new_pkg.name, &new_pkg.distro, &new_pkg.suite, Some(&new_pkg.architecture), connection)?;
            for mut pkg in pkgs {
                // TODO: this migration code is only necessary for a few releases
                if pkg.base_id.is_none() {
                    info!("updating base_id on {:?}/{:?}/{:?} {:?} -> {:?}", pkg.distro, pkg.suite, pkg.name, pkg.version, new_pkg.base_id);
                    pkg.base_id = new_pkg.base_id;
                    pkg.update(connection)?;
                }

                queue.push((pkg.id, pkg.version));
            }
        }
    }

    info!("updated_pkgs packages: {:?}", updated_pkgs.len());
    for (_, (base, mut pkg)) in updated_pkgs {
        let pkgbases = models::PkgBase::get_by(&base, &pkg.distro, &pkg.suite, Some(&pkg.architecture), connection)?
            .into_iter()
            .filter(|b| b.version == pkg.version)
            .collect::<Vec<_>>();

        if pkgbases.len() != 1 {
            bail!("Failed to locate pkgbase: {:?}/{:?}/{:?} ({:?}, {:?})", base, pkg.distro, pkg.suite, pkg.version, pkg.architecture);
        }

        let pkgbase = &pkgbases[0];
        pkg.base_id = Some(pkgbase.id);

        debug!("update: {:?}", pkg);
        pkg.bump_version(connection)?;
        queue.push((pkg.id, pkg.version.clone()));
    }

    info!("deleted packages: {:?}", deleted_pkgs.len());
    for (_, pkg) in deleted_pkgs {
        debug!("delete: {:?}", pkg);
        models::Package::delete(pkg.id, connection)?;
    }

    info!("queueing new jobs");
    // TODO: check if queueing has been disabled in the request, eg. to initially fill the database
    for pkgs in queue.chunks(1_000) {
        debug!("queue: {:?}", pkgs.len());
        models::Queued::queue_batch(pkgs, import.distro.to_string(), 1, connection)?;
    }
    info!("successfully updated state");

    Ok(())
}

fn retry(import: &SuiteImport, connection: &SqliteConnection) -> Result<()> {
    info!("selecting packages with due retries");
    let queue = models::Package::list_distro_suite_due_retries(import.distro.as_ref(), &import.suite, connection)?;

    info!("queueing new jobs");
    for pkgs in queue.chunks(1_000) {
        debug!("queue: {:?}", pkgs.len());
        models::Queued::queue_batch(pkgs, import.distro.to_string(), 2, connection)?;
    }
    info!("successfully triggered {} retries", queue.len());

    Ok(())
}

pub fn run(mut import: SuiteImport, connection: &SqliteConnection) -> Result<()> {
    sync(&mut import, connection)?;
    retry(&import, connection)?;
    Ok(())
}
