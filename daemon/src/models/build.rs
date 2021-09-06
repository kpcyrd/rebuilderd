use crate::schema::*;
use diesel::sql_types;
use diesel::prelude::*;
use rebuilderd_common::api::BuildReport;
use rebuilderd_common::errors::*;

#[derive(Identifiable, Queryable, AsChangeset, Clone, PartialEq, Debug)]
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
}

#[derive(Insertable, PartialEq, Debug, Clone)]
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

            if let Some(id) = rows.get(0) {
                Ok(*id)
            } else {
                bail!("Failed to get last_insert_id")
            }
        }).context("Failed to insert build to db")?;

        Ok(id)
    }

    pub fn from_api(report: &BuildReport) -> NewBuild {
        NewBuild {
            diffoscope: report.rebuild.diffoscope.clone(),
            attestation: report.rebuild.attestation.clone(),
            build_log: report.rebuild.log.as_bytes().to_vec(),
        }
    }
}
