use crate::diesel::ExpressionMethods;
use crate::diesel::QueryDsl;
use crate::models::SourcePackage;
use crate::schema::*;
use chrono::{Duration, NaiveDateTime, Utc};
use diesel::{
    AsChangeset, Associations, Identifiable, Insertable, Queryable, RunQueryDsl, Selectable,
    SelectableHelper, SqliteConnection,
};
use log::debug;
use rebuilderd_common::errors::*;

#[derive(
    Identifiable, Queryable, Selectable, Associations, AsChangeset, Clone, PartialEq, Eq, Debug,
)]
#[diesel(belongs_to(SourcePackage))]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[diesel(treat_none_as_null = true)]
#[diesel(table_name = build_inputs)]
pub struct BuildInput {
    pub id: i32,
    pub source_package_id: i32,
    pub url: String,
    pub backend: String,
    pub architecture: String,
    pub retries: i32,
    pub next_retry: Option<NaiveDateTime>,
}

impl BuildInput {
    pub fn schedule_retry(
        &mut self,
        retry_delay_base: i64,
        connection: &mut SqliteConnection,
    ) -> Result<()> {
        let hours = (self.retries as i64 + 1) * retry_delay_base;
        debug!("scheduling retry in {} hours", hours);

        let delay = Duration::hours(hours);
        self.next_retry = Some((Utc::now() + delay).naive_utc());

        self.update(connection)
    }

    pub fn clear_retry(&mut self, connection: &mut SqliteConnection) -> Result<()> {
        self.next_retry = None;

        self.update(connection)
    }

    pub fn update(&self, connection: &mut SqliteConnection) -> Result<()> {
        diesel::update(build_inputs::table.filter(build_inputs::id.eq(self.id)))
            .set(self)
            .execute(connection)?;

        Ok(())
    }
}

#[derive(Insertable, PartialEq, Eq, Debug, Clone)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[diesel(table_name = build_inputs)]
pub struct NewBuildInput {
    pub source_package_id: i32,
    pub url: String,
    pub backend: String,
    pub architecture: String,
    pub retries: i32,
}

impl NewBuildInput {
    pub fn upsert(&self, connection: &mut SqliteConnection) -> Result<BuildInput> {
        use crate::schema::build_inputs::*;

        let result = diesel::insert_into(table)
            .values(self)
            .on_conflict((source_package_id, url, backend, architecture))
            .do_update()
            .set((
                source_package_id.eq(diesel::upsert::excluded(source_package_id)),
                url.eq(diesel::upsert::excluded(url)),
                backend.eq(diesel::upsert::excluded(backend)),
                architecture.eq(diesel::upsert::excluded(architecture)),
            ))
            .returning(BuildInput::as_select())
            .get_result::<BuildInput>(connection)?;

        Ok(result)
    }
}
