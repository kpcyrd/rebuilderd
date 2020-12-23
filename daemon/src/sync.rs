use rebuilderd_common::errors::*;
use crate::models;
use rebuilderd_common::api::*;
use rebuilderd_common::PkgRelease;
use rebuilderd_common::Distro;
use std::collections::HashMap;
use crate::versions::PkgVerCmp;
use diesel::SqliteConnection;
use std::cmp::Ordering;

fn sync(import: &mut SuiteImport, connection: &SqliteConnection) -> Result<()> {
    info!("submitted packages {:?}", import.pkgs.len());

    // expand groups into individual packages
    let mut import_pkgs = Vec::new();
    for base in import.pkgs.drain(..) {
        for artifact in base.artifacts {
            import_pkgs.push(PkgRelease::new(
                artifact.name,
                base.version.clone(),
                import.distro,
                base.suite.clone(),
                base.architecture.clone(),
                artifact.url,
            ));
        }
    }

    // run regular import
    let mut pkgs = Vec::new();
    pkgs.extend(models::Package::list_distro_suite_architecture(import.distro.as_ref(), &import.suite, &import.architecture, connection)?);

    // TODO: come up with a better solution for this
    if import.distro == Distro::Archlinux {
        pkgs.extend(models::Package::list_distro_suite_architecture(import.distro.as_ref(), &import.suite, "any", connection)?);
    } else if import.distro == Distro::Debian {
        pkgs.extend(models::Package::list_distro_suite_architecture(import.distro.as_ref(), &import.suite, "all", connection)?);
    };

    let mut pkgs = pkgs.into_iter()
        .map(|pkg| (pkg.name.clone(), pkg))
        .collect::<HashMap<_, _>>();
    info!("existing packages {:?}", pkgs.len());

    let mut new_pkgs = HashMap::<_, PkgRelease>::new();
    let mut updated_pkgs = HashMap::<_, models::Package>::new();
    let mut deleted_pkgs = pkgs.clone();

    for pkg in import_pkgs.drain(..) {
        deleted_pkgs.remove_entry(&pkg.name);

        if let Some(cur) = new_pkgs.get_mut(&pkg.name) {
            cur.bump_package(&import.distro, &pkg)?;
        } else if let Some(cur) = updated_pkgs.get_mut(&pkg.name) {
            cur.bump_package(&import.distro, &pkg)?;
        } else if let Some(old) = pkgs.get_mut(&pkg.name) {
            if old.bump_package(&import.distro, &pkg)? == Ordering::Greater {
                updated_pkgs.insert(pkg.name, old.clone());
            }
        } else {
            new_pkgs.insert(pkg.name.clone(), pkg);
        }
    }

    // TODO: consider starting a transaction here
    let mut queue = Vec::<(i32, String)>::new();

    // TODO: if the package is queued, don't queue it again. Right now we can't rebuild the non-latest version anyway

    info!("new packages: {:?}", new_pkgs.len());
    let new_pkgs = new_pkgs.into_iter()
        .map(|(_, v)| models::NewPackage::from_api(import.distro, v))
        .collect::<Vec<_>>();
    for pkgs in new_pkgs.chunks(1_000) {
        debug!("new: {:?}", pkgs.len());
        models::NewPackage::insert_batch(pkgs, connection)?;

        // this is needed because diesel doesn't return ids when inserting into sqlite
        // this is obviously slow and needs to be refactored
        for pkg in pkgs {
            let pkgs = models::Package::get_by(&pkg.name, &pkg.distro, &pkg.suite, Some(&pkg.architecture), connection)?;
            for pkg in pkgs {
                queue.push((pkg.id, pkg.version));
            }
        }
    }

    info!("updated_pkgs packages: {:?}", updated_pkgs.len());
    for (_, mut pkg) in updated_pkgs {
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
        models::Queued::queue_batch(pkgs, 1, connection)?;
    }
    info!("successfully updated state");

    Ok(())
}

fn retry(import: &SuiteImport, connection: &SqliteConnection) -> Result<()> {
    info!("selecting packages with due retries");
    let queue = models::Package::list_distro_suite_architecture_due_retries(import.distro.as_ref(), &import.suite, &import.architecture, connection)?;

    info!("queueing new jobs");
    for pkgs in queue.chunks(1_000) {
        debug!("queue: {:?}", pkgs.len());
        models::Queued::queue_batch(pkgs, 2, connection)?;
    }
    info!("successfully triggered {} retries", queue.len());

    Ok(())
}

pub fn run(mut import: SuiteImport, connection: &SqliteConnection) -> Result<()> {
    sync(&mut import, connection)?;
    retry(&import, connection)?;
    Ok(())
}
