use chrono::{Utc, NaiveDateTime, Duration};
use crate::schema::*;
use diesel::prelude::*;
use rebuilderd_common::{PkgRelease, Status};
use rebuilderd_common::api::{Rebuild, BuildStatus};
use rebuilderd_common::errors::*;

#[derive(Identifiable, Queryable, AsChangeset, Clone, PartialEq, Debug)]
#[changeset_options(treat_none_as_null="true")]
#[table_name="packages"]
pub struct Package {
    pub id: i32,
    pub base_id: Option<i32>,
    pub name: String,
    pub version: String,
    pub status: String,
    pub distro: String,
    pub suite: String,
    pub architecture: String,
    pub artifact_url: String,
    pub input_url: Option<String>,
    pub build_id: Option<i32>,
    pub built_at: Option<NaiveDateTime>,
    pub has_diffoscope: bool,
    pub has_attestation: bool,
    pub checksum: Option<String>,
    pub retries: i32,
    pub next_retry: Option<NaiveDateTime>,
}

impl Package {
    pub fn get_id(my_id: i32, connection: &SqliteConnection) -> Result<Package> {
        use crate::schema::packages::dsl::*;
        let pkg = packages
            .filter(id.eq(my_id))
            .first::<Package>(connection)?;
        Ok(pkg)
    }

    pub fn get_by(my_name: &str, my_distro: &str, my_suite: &str, my_architecture: Option<&str>, connection: &SqliteConnection) -> Result<Vec<Package>> {
        use crate::schema::packages::dsl::*;
        let mut query = packages
            .filter(name.eq(my_name))
            .filter(distro.eq(my_distro))
            .filter(suite.eq(my_suite))
            .into_boxed();
        if let Some(my_architecture) = my_architecture {
            query = query.filter(architecture.eq(my_architecture));
        }
        let pkg = query.load::<Package>(connection)?;
        Ok(pkg)
    }

    pub fn get_by_api(pkg: &PkgRelease, connection: &SqliteConnection) -> Result<Package> {
        use crate::schema::packages::dsl::*;
        let pkg = packages
            .filter(name.eq(&pkg.name))
            .filter(version.eq(&pkg.version))
            .filter(distro.eq(&pkg.distro))
            .filter(suite.eq(&pkg.suite))
            .filter(architecture.eq(&pkg.architecture))
            .first::<Package>(connection)?;
        Ok(pkg)
    }

    pub fn list(connection: &SqliteConnection) -> Result<Vec<Package>> {
        use crate::schema::packages::dsl::*;
        let pkgs = packages
            .order_by((name, distro))
            .load::<Package>(connection)?;
        Ok(pkgs)
    }

    pub fn list_distro_suite(my_distro: &str, my_suite: &str, connection: &SqliteConnection) -> Result<Vec<Package>> {
        use crate::schema::packages::dsl::*;
        let pkgs = packages
            .filter(distro.eq(my_distro))
            .filter(suite.eq(my_suite))
            .load::<Package>(connection)?;
        Ok(pkgs)
    }

    pub fn list_distro_suite_due_retries(my_distro: &str, my_suite: &str, connection: &SqliteConnection) -> Result<Vec<(i32, String)>> {
        use crate::schema::packages::dsl::*;
        use crate::schema::queue;
        let pkgs = packages
            .select((id, version))
            .filter(distro.eq(my_distro))
            .filter(suite.eq(my_suite))
            .filter(next_retry.le(Utc::now().naive_utc()))
            .left_outer_join(queue::table.on(id.eq(queue::package_id)))
            .filter(queue::id.is_null())
            .load(connection)?;
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

    pub fn bump_version(&mut self, connection: &SqliteConnection) -> Result<()> {
        self.status = Status::Unknown.to_string();
        self.built_at = None;
        self.retries = 0;
        self.next_retry = None;

        diesel::update(&*self)
            .set(&*self)
            .execute(connection)?;

        Ok(())
    }

    pub fn schedule_retry(&mut self, retry_delay_base: i64) {
        let hours = (self.retries as i64 + 1) * retry_delay_base;
        debug!("scheduling retry in {} hours", hours);
        let delay = Duration::hours(hours);
        self.next_retry = Some((Utc::now() + delay).naive_utc());
    }

    pub fn update_status_safely(&mut self, rebuild: &Rebuild, connection: &SqliteConnection) -> Result<()> {
        use crate::schema::packages::columns::*;

        if self.status == *Status::Bad {
            self.retries += 1;
        }

        self.status = match rebuild.status {
            BuildStatus::Good => Status::Good.to_string(),
            _ => Status::Bad.to_string(),
        };
        self.built_at = Some(Utc::now().naive_utc());
        self.has_attestation = rebuild.attestation.is_some();
        diesel::update(packages::table
                .filter(id.eq(self.id))
                .filter(version.eq(&self.version))
            )
            .set(&*self)
            .execute(connection)?;
        Ok(())
    }

    pub fn reset_status_for_requeued_list(pkgs: &[i32], connection: &SqliteConnection) -> Result<()> {
        use crate::schema::packages::columns::*;
        diesel::update(packages::table
                .filter(id.eq_any(pkgs))
            )
            .set((
                status.eq("UNKWN"),
                build_id.eq(None as Option<i32>),
            ))
            .execute(connection)?;
        Ok(())
    }

    pub fn delete(my_id: i32, connection: &SqliteConnection) -> Result<()> {
        use crate::schema::packages::dsl::*;
        diesel::delete(packages.filter(id.eq(my_id)))
            .execute(connection)?;
        Ok(())
    }

    pub fn most_recent_built_at(connection: &SqliteConnection) -> Result<Option<NaiveDateTime>> {
        use crate::schema::packages::dsl::*;

        let latest_built = packages.select(diesel::dsl::max(built_at)).first(connection)?;

        Ok(latest_built)
    }

    pub fn into_api_item(self) -> Result<PkgRelease> {
        Ok(PkgRelease {
            name: self.name,
            distro: self.distro,
            architecture: self.architecture,
            version: self.version,
            status: self.status.parse()?,
            suite: self.suite,
            artifact_url: self.artifact_url,
            input_url: self.input_url,
            build_id: self.build_id,
            built_at: self.built_at,
            has_diffoscope: self.has_diffoscope,
            has_attestation: self.has_attestation,
            next_retry: self.next_retry,
        })
    }
}

#[derive(Insertable, PartialEq, Debug, Clone)]
#[table_name="packages"]
pub struct NewPackage {
    pub base_id: Option<i32>,
    pub name: String,
    pub version: String,
    pub status: String,
    pub distro: String,
    pub suite: String,
    pub architecture: String,
    pub artifact_url: String,
    pub input_url: Option<String>,
    pub build_id: Option<i32>,
    pub built_at: Option<NaiveDateTime>,
    pub has_diffoscope: bool,
    pub has_attestation: bool,
    pub checksum: Option<String>,
    pub retries: i32,
    pub next_retry: Option<NaiveDateTime>,
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

    pub fn from_api(distro: String, base_id: i32, pkg: PkgRelease) -> NewPackage {
        NewPackage {
            base_id: Some(base_id),
            name: pkg.name,
            version: pkg.version,
            status: pkg.status.to_string(),
            distro,
            suite: pkg.suite,
            architecture: pkg.architecture,
            artifact_url: pkg.artifact_url,
            input_url: pkg.input_url,
            build_id: None,
            built_at: None,
            has_diffoscope: false,
            has_attestation: false,
            checksum: None,
            retries: 0,
            next_retry: None,
        }
    }
}
