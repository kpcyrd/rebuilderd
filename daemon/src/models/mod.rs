use crate::schema::*;
use rebuilderd_common::api;
use rebuilderd_common::errors::*;
use diesel::prelude::*;
use chrono::prelude::*;
// use std::net::IpAddr;
use serde::{Serialize, Deserialize};
use std::net::IpAddr;
use rebuilderd_common::PkgRelease;
use rebuilderd_common::Distro;

#[derive(Identifiable, Queryable, AsChangeset, Clone, PartialEq, Debug)]
#[table_name="packages"]
pub struct Package {
    pub id: i32,
    pub name: String,
    pub version: String,
    pub status: String,
    pub distro: String,
    pub suite: String,
    pub architecture: String,
    pub url: String,
}

impl Package {
    pub fn list(connection: &SqliteConnection) -> Result<Vec<Package>> {
        use crate::schema::packages::dsl::*;
        let pkgs = packages
            .order_by((name, distro))
            .load::<Package>(connection)?;
        Ok(pkgs)
    }

    pub fn list_distro_suite_architecture(my_distro: &str, my_suite: &str, my_architecture: &str, connection: &SqliteConnection) -> Result<Vec<Package>> {
        use crate::schema::packages::dsl::*;
        let pkgs = packages
            .filter(distro.eq(my_distro))
            .filter(suite.eq(my_suite))
            .filter(architecture.eq(my_architecture))
            .load::<Package>(connection)?;
        Ok(pkgs)
    }

    // when updating the verify status, use a custom query that enforces a version match
    pub fn update(&self, connection: &SqliteConnection) -> Result<()> {
        use crate::schema::packages::columns::*;
        diesel::update(packages::table.filter(id.eq(self.id)))
            .set(self)
            .execute(connection)?;
        Ok(())
    }

    pub fn delete(my_id: i32, connection: &SqliteConnection) -> Result<()> {
        use crate::schema::packages::dsl::*;
        diesel::delete(packages.filter(id.eq(my_id)))
            .execute(connection)?;
        Ok(())
    }
}

#[derive(Insertable, PartialEq, Debug)]
#[table_name="packages"]
pub struct NewPackage {
    pub name: String,
    pub version: String,
    pub status: String,
    pub distro: String,
    pub suite: String,
    pub architecture: String,
    pub url: String,
}

impl NewPackage {
    pub fn insert(&self, connection: &SqliteConnection) -> Result<()> {
        diesel::insert_into(packages::table)
            .values(self)
            .execute(connection)?;
        Ok(())
    }

    pub fn insert_batch(pkgs: &[NewPackage], connection: &SqliteConnection) -> Result<()> {
        diesel::insert_into(packages::table)
            .values(pkgs)
            .execute(connection)?;
        Ok(())
    }

    pub fn from_api(distro: Distro, pkg: PkgRelease) -> NewPackage {
        NewPackage {
            name: pkg.name,
            version: pkg.version,
            status: pkg.status.to_string(),
            distro: distro.to_string(),
            suite: pkg.suite,
            architecture: pkg.architecture,
            url: pkg.url,
        }
    }
}

#[derive(Identifiable, Queryable, AsChangeset, Serialize, PartialEq, Debug)]
#[table_name="workers"]
pub struct Worker {
    pub id: i32,
    pub key: String,
    // TODO: pub addr: IpAddr,
    pub addr: String,
    pub status: Option<String>,
    pub last_ping: NaiveDateTime,
    pub online: bool,
}

impl Worker {
    pub fn get(my_key: &str, connection: &SqliteConnection) -> Result<Option<Worker>> {
        use crate::schema::workers::dsl::*;
        let worker = workers.filter(key.eq(my_key))
            .first::<Worker>(connection)
            .optional()?;
        Ok(worker)
    }

    pub fn list(connection: &SqliteConnection) -> Result<Vec<Worker>> {
        use crate::schema::workers::dsl::*;
        let results = workers.filter(online.eq(true))
            .load::<Worker>(connection)?;
        Ok(results)
    }
}

impl From<Worker> for api::Worker {
    fn from(worker: Worker) -> api::Worker {
        api::Worker {
            key: worker.key,
            addr: worker.addr,
            status: worker.status,
            last_ping: worker.last_ping,
            online: worker.online,
        }
    }
}

#[derive(Insertable, Serialize, Deserialize, Debug)]
#[table_name="workers"]
pub struct NewWorker {
    pub key: String,
    pub addr: String,
    pub status: Option<String>,
    pub last_ping: NaiveDateTime,
    pub online: bool,
}


impl NewWorker {
    pub fn insert(&self, connection: &SqliteConnection) -> Result<()> {
        if Worker::get(&self.key, connection)?.is_some() {
            return Ok(());
        }

        diesel::insert_into(workers::table)
            .values(self)
            .execute(connection)?;
        Ok(())
    }

    pub fn new(query: api::WorkQuery, addr: IpAddr, status: Option<String>) -> NewWorker {
        let now: DateTime<Utc> = Utc::now();

        NewWorker {
            key: query.key,
            addr: addr.to_string(),
            status,
            last_ping: now.naive_utc(),
            online: true,
        }
    }
}
