#![allow(clippy::extra_unused_lifetimes)]

use crate::schema::*;
use diesel::sql_types;
use diesel::prelude::*;
use diesel::sql_types::Integer;
use rebuilderd_common::api::Rebuild;
use rebuilderd_common::errors::*;

#[derive(Identifiable, Queryable, AsChangeset, Clone, PartialEq, Eq, Debug)]
#[table_name="builds"]
pub struct Build {
    pub id: i32,
    pub diffoscope: Option<String>,
    pub build_log: Vec<u8>,
    pub attestation: Option<String>,
}

impl Build {
    pub fn get_id(my_id: i32, connection: &SqliteConnection) -> Result<Build> {
        use crate::schema::builds::dsl::*;
        let build = builds
            .filter(id.eq(my_id))
            .first::<Build>(connection)?;
        Ok(build)
    }

    pub fn find_orphaned(connection: &SqliteConnection) -> Result<Vec<i32>> {
        let ids = diesel::sql_query("select id from builds as b where not exists (select 1 from packages as p where p.build_id = b.id);")
            .load::<IdRow>(connection)?;
        let ids = ids.into_iter().map(|x| x.id).collect();
        Ok(ids)
    }

    pub fn delete_multiple(ids: &[i32], connection: &SqliteConnection) -> Result<()> {
        use crate::schema::builds::dsl::*;
        diesel::delete(builds.filter(id.eq_any(ids))).execute(connection)?;
        Ok(())
    }
}

#[derive(Debug, QueryableByName)]
struct IdRow {
    #[sql_type = "Integer"]
    id: i32,
}

#[derive(Insertable, PartialEq, Eq, Debug, Clone)]
#[table_name="builds"]
pub struct NewBuild {
    pub diffoscope: Option<String>,
    pub build_log: Vec<u8>,
    pub attestation: Option<String>,
}

impl NewBuild {
    pub fn insert(&self, connection: &SqliteConnection) -> Result<i32> {
        let id = connection.transaction::<_, Error, _>(|| {
            diesel::insert_into(builds::table)
                .values(self)
                .execute(connection)?;

            no_arg_sql_function!(last_insert_rowid, sql_types::Integer);
            let rows = diesel::select(last_insert_rowid).load::<i32>(connection)?;

            if let Some(id) = rows.first() {
                Ok(*id)
            } else {
                bail!("Failed to get last_insert_id")
            }
        }).context("Failed to insert build to db")?;

        Ok(id)
    }

    pub fn from_api(rebuild: &Rebuild, build_log: Vec<u8>) -> NewBuild {
        NewBuild {
            diffoscope: rebuild.diffoscope.clone(),
            attestation: rebuild.attestation.clone(),
            build_log,
        }
    }
}
