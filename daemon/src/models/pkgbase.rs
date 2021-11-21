use chrono::{NaiveDateTime, Utc};
use crate::schema::*;
use diesel::prelude::*;
use rebuilderd_common::PkgGroup;
use rebuilderd_common::errors::*;

#[derive(Identifiable, Queryable, AsChangeset, Clone, PartialEq, Debug)]
#[table_name="pkgbases"]
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
    pub fn list_distro_suite(my_distro: &str, my_suite: &str, connection: &SqliteConnection) -> Result<Vec<PkgBase>> {
        use crate::schema::pkgbases::dsl::*;
        let bases = pkgbases
            .filter(distro.eq(my_distro))
            .filter(suite.eq(my_suite))
            .load::<PkgBase>(connection)?;
        Ok(bases)
    }

    pub fn list_pkgs(&self, connection: &SqliteConnection) -> Result<Vec<i32>> {
        use crate::schema::packages::dsl::*;
        let pkgs = packages
            .select(id)
            .filter(base_id.eq(self.id))
            .load(connection)?;
        Ok(pkgs)
    }

    pub fn get_id(my_id: i32, connection: &SqliteConnection) -> Result<PkgBase> {
        use crate::schema::pkgbases::dsl::*;
        let pkgbase = pkgbases
            .filter(id.eq(my_id))
            .first::<PkgBase>(connection)?;
        Ok(pkgbase)
    }

    pub fn get_by(my_name: &str, my_distro: &str, my_suite: &str, my_architecture: Option<&str>, connection: &SqliteConnection) -> Result<Vec<PkgBase>> {
        use crate::schema::pkgbases::dsl::*;
        let mut query = pkgbases
            .filter(name.eq(my_name))
            .filter(distro.eq(my_distro))
            .filter(suite.eq(my_suite))
            .into_boxed();
        if let Some(my_architecture) = my_architecture {
            query = query.filter(architecture.eq(my_architecture));
        }
        let pkg = query.load::<PkgBase>(connection)?;
        Ok(pkg)
    }

    pub fn list_distro_suite_due_retries(my_distro: &str, my_suite: &str, connection: &SqliteConnection) -> Result<Vec<(i32, String)>> {
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

    pub fn delete(my_id: i32, connection: &SqliteConnection) -> Result<()> {
        use crate::schema::pkgbases::dsl::*;
        diesel::delete(pkgbases.filter(id.eq(my_id)))
            .execute(connection)?;
        Ok(())
    }
}

#[derive(Insertable, PartialEq, Debug, Clone)]
#[table_name="pkgbases"]
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
    pub fn insert(&self, connection: &SqliteConnection) -> Result<()> {
        diesel::insert_into(pkgbases::table)
            .values(self)
            .execute(connection)?;
        Ok(())
    }

    pub fn insert_batch(pkgs: &[NewPkgBase], connection: &SqliteConnection) -> Result<()> {
        diesel::insert_into(pkgbases::table)
            .values(pkgs)
            .execute(connection)?;
        Ok(())
    }
}
