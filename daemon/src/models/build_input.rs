use crate::models::SourcePackage;
use crate::schema::*;
use chrono::NaiveDateTime;
use diesel::ExpressionMethods;
use diesel::{
    AsChangeset, Associations, Identifiable, Insertable, Queryable, RunQueryDsl, Selectable,
    SelectableHelper, SqliteConnection,
};
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

#[derive(Insertable, PartialEq, Eq, Debug, Clone)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[diesel(table_name = build_inputs)]
pub struct NewBuildInput {
    pub source_package_id: i32,
    pub url: String,
    pub backend: String,
    pub architecture: String,
    pub retries: i32,
    pub next_retry: Option<NaiveDateTime>,
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
