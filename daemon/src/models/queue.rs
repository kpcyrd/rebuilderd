use crate::schema::*;
use chrono::prelude::*;
use diesel::prelude::*;
use diesel::upsert::excluded;
use rebuilderd_common::errors::*;
use serde::{Deserialize, Serialize};

#[derive(Identifiable, Queryable, Selectable, AsChangeset, Serialize, PartialEq, Eq, Debug)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[diesel(treat_none_as_null = true)]
#[diesel(table_name = queue)]
pub struct Queued {
    pub id: i32,
    pub build_input_id: i32,
    pub priority: i32,
    pub queued_at: NaiveDateTime,
    pub started_at: Option<NaiveDateTime>,
    pub worker: Option<i32>,
    pub last_ping: Option<NaiveDateTime>,
}

impl Queued {
    pub fn delete(&self, connection: &mut SqliteConnection) -> Result<()> {
        use crate::schema::queue::columns::*;
        diesel::delete(queue::table.filter(id.is(self.id))).execute(connection)?;
        Ok(())
    }
}

#[derive(Insertable, Serialize, Deserialize, Debug)]
#[diesel(table_name = queue)]
pub struct NewQueued {
    pub build_input_id: i32,
    pub priority: i32,
    pub queued_at: NaiveDateTime,
}

impl NewQueued {
    // TODO: upserting isn't quite right here... we only upsert some fields and that's not consistent
    pub fn upsert(&self, connection: &mut SqliteConnection) -> Result<Queued> {
        use crate::schema::queue::*;

        let result = diesel::insert_into(table)
            .values(self)
            .on_conflict(build_input_id)
            .do_update()
            .set((
                build_input_id.eq(excluded(build_input_id)),
                priority.eq(excluded(priority)),
            ))
            .returning(Queued::as_select())
            .get_result::<Queued>(connection)?;

        Ok(result)
    }
}
