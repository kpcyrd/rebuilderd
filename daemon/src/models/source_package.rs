use crate::schema::*;
use chrono::NaiveDateTime;
use diesel::prelude::*;
use diesel::sql_types::Text;
use rebuilderd_common::errors::*;

#[derive(Identifiable, Queryable, Selectable, AsChangeset, Clone, PartialEq, Eq, Debug)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[diesel(treat_none_as_null = true)]
#[diesel(table_name = source_packages)]
pub struct SourcePackage {
    pub id: i32,
    pub name: String,
    pub version: String,
    pub distribution: String,
    pub release: Option<String>,
    pub component: Option<String>,
    pub last_seen: NaiveDateTime,
    pub seen_in_last_sync: bool,
}

#[derive(Insertable, AsChangeset, PartialEq, Eq, Debug, Clone)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[diesel(treat_none_as_null = true)]
#[diesel(treat_none_as_default_value = false)]
#[diesel(table_name = source_packages)]
pub struct NewSourcePackage {
    pub name: String,
    pub version: String,
    pub distribution: String,
    pub release: Option<String>,
    pub component: Option<String>,
    pub last_seen: NaiveDateTime,
    pub seen_in_last_sync: bool,
}

impl NewSourcePackage {
    pub fn upsert(&self, connection: &mut SqliteConnection) -> Result<SourcePackage> {
        diesel::insert_into(source_packages::table)
            .values(self)
            .on_conflict(diesel::dsl::sql::<Text>(""))
            .do_update()
            .set(self)
            .returning(SourcePackage::as_select())
            .get_result::<SourcePackage>(connection)
            .map_err(Error::from)
    }
}
