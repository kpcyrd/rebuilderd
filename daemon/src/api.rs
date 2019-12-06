use actix_web::{get, post, web, HttpRequest, Responder};
use rebuilderd_common::errors::*;
use rebuilderd_common::Status;
use crate::models;
use rebuilderd_common::api::*;
use rebuilderd_common::PkgRelease;
use rebuilderd_common::Distro;
use crate::db::Pool;
use std::collections::HashMap;
use crate::versions::PkgVerCmp;
use std::cmp::Ordering;
use std::rc::Rc;

#[get("/api/v0/workers")]
pub async fn list_workers(
    pool: web::Data<Pool>,
) -> impl Responder {
    let connection = pool.get().unwrap();
    let workers = models::Worker::list(connection.as_ref()).unwrap(); // TODO
    web::Json(workers)
}

#[post("/api/v0/job/fetch")]
pub async fn fetch_work(
    req: HttpRequest,
    query: web::Json<WorkQuery>,
    pool: web::Data<Pool>,
) -> impl Responder {
    let connection = pool.get().unwrap();
    let status = None;

    let ci = req.peer_addr().expect("can't determine client ip");
    let worker = models::NewWorker::new(query.into_inner(), ci.ip(), status);
    worker.insert(connection.as_ref()).unwrap();

    web::Json(JobAssignment::Nothing)
}

use diesel::SqliteConnection;

fn run_sync(mut import: SuiteImport, connection: &SqliteConnection) -> Result<()> {
    info!("submitted packages {:?}", import.pkgs.len());
    let mut pkgs = Vec::new();
    pkgs.extend(models::Package::list_distro_suite_architecture(import.distro.as_ref(), &import.suite, &import.architecture, connection)?);

    // TODO: come up with a better solution for this
    if import.distro == Distro::Archlinux {
        pkgs.extend(models::Package::list_distro_suite_architecture(import.distro.as_ref(), &import.suite, "any", connection)?);
    } else if import.distro == Distro::Debian {
        pkgs.extend(models::Package::list_distro_suite_architecture(import.distro.as_ref(), &import.suite, "all", connection)?);
    };

    let mut pkgs = pkgs.into_iter()
        .map(|pkg| (pkg.name.clone(), Rc::new(pkg)))
        .collect::<HashMap<_, _>>();
    info!("existing packages {:?}", pkgs.len());

    let mut new_pkgs = HashMap::<_, PkgRelease>::new();
    let mut updated_pkgs = HashMap::<_, Rc<models::Package>>::new();
    let mut deleted_pkgs = pkgs.clone();

    // TODO: this loop is very slow because debian has to shell out to dpkg --compare-versions
    for pkg in import.pkgs.drain(..) {
        deleted_pkgs.remove_entry(&pkg.name);

        if let Some(cur) = new_pkgs.get_mut(&pkg.name) {
            cur.bump_package(&import.distro, &pkg)?;
        } else if let Some(cur) = updated_pkgs.get_mut(&pkg.name) {
            Rc::get_mut(cur).unwrap().bump_package(&import.distro, &pkg)?;
        } else if let Some(old) = pkgs.get_mut(&pkg.name) {
            let old2 = Rc::get_mut(old).unwrap();
            if old2.bump_package(&import.distro, &pkg)? == Ordering::Greater {
                updated_pkgs.insert(pkg.name, old.clone());
            }
        } else {
            new_pkgs.insert(pkg.name.clone(), pkg);
        }
    }

    info!("new packages: {:?}", new_pkgs.len());
    let new_pkgs = new_pkgs.into_iter()
        .map(|(_, v)| models::NewPackage::from_api(import.distro, v))
        .collect::<Vec<_>>();
    for pkgs in new_pkgs.chunks(1_000) {
        debug!("new: {:?}", pkgs.len());
        models::NewPackage::insert_batch(pkgs, connection)?;
    }

    info!("updated_pkgs packages: {:?}", updated_pkgs.len());
    for (_, pkg) in updated_pkgs {
        debug!("update: {:?}", pkg);
        pkg.update(connection)?;
    }

    info!("deleted packages: {:?}", deleted_pkgs.len());
    for (_, pkg) in deleted_pkgs {
        debug!("delete: {:?}", pkg);
        models::Package::delete(pkg.id, connection)?;
    }
    info!("successfully updated state");

    Ok(())
}

// #[post("/api/v0/job/sync")]
pub async fn sync_work(
    import: web::Json<SuiteImport>,
    pool: web::Data<Pool>,
) -> impl Responder {
    let import = import.into_inner();
    let connection = pool.get().unwrap();

    run_sync(import, connection.as_ref()).unwrap();

    web::Json(JobAssignment::Nothing)
}


fn opt_filter(this: &String, filter: &Option<String>) -> bool {
    if let Some(filter) = filter {
        if this != filter {
            return true;
        }
    }
    false
}

#[post("/api/v0/pkgs/list")]
pub async fn list_pkgs(
    query: web::Json<ListPkgs>,
    pool: web::Data<Pool>,
) -> impl Responder {
    let connection = pool.get().unwrap();

    let mut pkgs = Vec::<PkgRelease>::new();
    for pkg in models::Package::list(connection.as_ref()).unwrap() {
        if opt_filter(&pkg.name, &query.name) {
            continue;
        }
        if opt_filter(&pkg.distro, &query.distro) {
            continue;
        }
        if opt_filter(&pkg.suite, &query.suite) {
            continue;
        }
        if opt_filter(&pkg.architecture, &query.architecture) {
            continue;
        }

        let status = pkg.status.parse::<Status>().unwrap();

        pkgs.push(PkgRelease {
             name: pkg.name,
             version: pkg.version,
             status: status,
             distro: pkg.distro,
             suite: pkg.suite,
             architecture: pkg.architecture,
             url: pkg.url,
        });
    }

    web::Json(pkgs)
}
