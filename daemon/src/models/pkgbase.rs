use chrono::{Duration, NaiveDateTime, Utc};
use crate::schema::*;
use diesel::prelude::*;
use rebuilderd_common::PkgGroup;
use rebuilderd_common::errors::*;

#[derive(Identifiable, Queryable, AsChangeset, Clone, PartialEq, Eq, Debug)]
#[diesel(table_name = pkgbases)]
pub struct PkgBase {
    pub id: i32,
    pub name: String,
    pub version: String,
    pub distro: String,
    pub suite: String,
    pub architecture: String,
    pub input_url: Option<String>,
    pub artifacts: String,
    pub retries: i32,
    pub next_retry: Option<NaiveDateTime>,
}

impl PkgBase {
    pub fn list_distro_suite(my_distro: &str, my_suite: &str, connection: &mut SqliteConnection) -> Result<Vec<PkgBase>> {
        use crate::schema::pkgbases::dsl::*;
        let bases = pkgbases
            .filter(distro.eq(my_distro))
            .filter(suite.eq(my_suite))
            .load::<PkgBase>(connection)?;
        Ok(bases)
    }

    pub fn list_pkgs(&self, connection: &mut SqliteConnection) -> Result<Vec<i32>> {
        use crate::schema::packages::dsl::*;
        let pkgs = packages
            .select(id)
            .filter(pkgbase_id.eq(self.id))
            .load(connection)?;
        Ok(pkgs)
    }

    pub fn get_id(my_id: i32, connection: &mut SqliteConnection) -> Result<PkgBase> {
        use crate::schema::pkgbases::dsl::*;
        let pkgbase = pkgbases
            .filter(id.eq(my_id))
            .first::<PkgBase>(connection)?;
        Ok(pkgbase)
    }

    pub fn get_id_list(my_ids: &[i32], connection: &mut SqliteConnection) -> Result<Vec<PkgBase>> {
        use crate::schema::pkgbases::dsl::*;
        let pkgbase = pkgbases
            .filter(id.eq_any(my_ids))
            .load::<PkgBase>(connection)?;
        Ok(pkgbase)
    }

    pub fn get_by(my_name: &str, my_distro: &str, my_suite: &str, my_version: Option<&str>, my_architecture: Option<&str>, connection: &mut SqliteConnection) -> Result<Vec<PkgBase>> {
        use crate::schema::pkgbases::dsl::*;
        let mut query = pkgbases
            .filter(name.eq(my_name))
            .filter(distro.eq(my_distro))
            .filter(suite.eq(my_suite))
            .into_boxed();
        if let Some(my_version) = my_version {
            query = query.filter(version.eq(my_version));
        }
        if let Some(my_architecture) = my_architecture {
            query = query.filter(architecture.eq(my_architecture));
        }
        Ok(query.load::<PkgBase>(connection)?)
    }

    pub fn list_distro_suite_due_retries(my_distro: &str, my_suite: &str, connection: &mut SqliteConnection) -> Result<Vec<(i32, String)>> {
        use crate::schema::pkgbases::dsl::*;
        use crate::schema::queue;
        let pkgs = pkgbases
            .select((id, version))
            .filter(distro.eq(my_distro))
            .filter(suite.eq(my_suite))
            .filter(next_retry.le(Utc::now().naive_utc()))
            .left_outer_join(queue::table.on(id.eq(queue::pkgbase_id)))
            .filter(queue::id.is_null())
            .load(connection)?;
        Ok(pkgs)
    }

    pub fn schedule_retry(&mut self, retry_delay_base: i64) {
        let hours = (self.retries as i64 + 1) * retry_delay_base;
        debug!("scheduling retry in {} hours", hours);
        let delay = Duration::hours(hours);
        self.next_retry = Some((Utc::now() + delay).naive_utc());
    }

    pub fn clear_retry(&mut self, connection: &mut SqliteConnection) -> Result<()> {
        use crate::schema::pkgbases::columns::*;
        self.next_retry = None;
        diesel::update(pkgbases::table.filter(id.eq(self.id)))
            .set((
                next_retry.eq(None as Option::<NaiveDateTime>),
            ))
            .execute(connection)?;
        Ok(())
    }

    pub fn into_api_item(self) -> Result<PkgGroup> {
        let artifacts = serde_json::from_str(&self.artifacts).expect("Failed to deserialize artifact");

        Ok(PkgGroup {
            name: self.name,
            version: self.version,

            distro: self.distro,
            suite: self.suite,
            architecture: self.architecture,

            input_url: self.input_url,
            artifacts,
        })
    }

    pub fn update(&self, connection: &mut SqliteConnection) -> Result<()> {
        use crate::schema::pkgbases::columns::*;
        diesel::update(pkgbases::table.filter(id.eq(self.id)))
            .set(self)
            .execute(connection)?;
        Ok(())
    }

    pub fn delete(my_id: i32, connection: &mut SqliteConnection) -> Result<()> {
        use crate::schema::pkgbases::dsl::*;
        diesel::delete(pkgbases.filter(id.eq(my_id)))
            .execute(connection)?;
        Ok(())
    }

    pub fn delete_batch(batch: &[i32], connection: &mut SqliteConnection) -> Result<()> {
        use crate::schema::pkgbases::dsl::*;
        diesel::delete(pkgbases.filter(id.eq_any(batch)))
            .execute(connection)?;
        Ok(())
    }
}

#[derive(Insertable, PartialEq, Eq, Debug, Clone)]
#[diesel(table_name = pkgbases)]
pub struct NewPkgBase {
    pub name: String,
    pub version: String,
    pub distro: String,
    pub suite: String,
    pub architecture: String,
    pub input_url: Option<String>,
    pub artifacts: String,
    pub retries: i32,
    pub next_retry: Option<NaiveDateTime>,
}

impl NewPkgBase {
    pub fn insert(&self, connection: &mut SqliteConnection) -> Result<()> {
        diesel::insert_into(pkgbases::table)
            .values(self)
            .execute(connection)?;
        Ok(())
    }

    pub fn insert_batch(pkgs: &[NewPkgBase], connection: &mut SqliteConnection) -> Result<()> {
        diesel::insert_into(pkgbases::table)
            .values(pkgs)
            .execute(connection)?;
        Ok(())
    }
}
