use crate::schema::*;
use chrono::prelude::*;
use diesel::prelude::*;
use diesel::upsert::excluded;
use rebuilderd_common::errors::*;
use serde::{Deserialize, Serialize};

#[derive(Identifiable, Queryable, AsChangeset, Selectable, Serialize, PartialEq, Eq, Debug)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[diesel(treat_none_as_null = true)]
#[diesel(table_name = workers)]
pub struct Worker {
    pub id: i32,
    pub name: String,
    pub key: String,
    pub address: String,
    pub status: Option<String>,
    pub last_ping: NaiveDateTime,
    pub online: bool,
}

impl Worker {
    pub fn get_and_refresh(key: &str, connection: &mut SqliteConnection) -> Result<Worker> {
        let worker = diesel::update(workers::table.filter(workers::key.is(key)))
            .set((
                workers::last_ping.eq(Utc::now().naive_utc()),
                workers::online.eq(true),
            ))
            .returning(Worker::as_select())
            .get_result(connection)?;

        Ok(worker)
    }

    pub fn get_or_create(key: &str, name: &str, connection: &mut SqliteConnection) -> Result<Worker> {
        let now = Utc::now().naive_utc();
        let new_worker = NewWorker {
            key: key.to_string(),
            name: name.to_string(),
            address: "unauthenticated".to_string(),
            status: None,
            last_ping: now,
            online: true,
        };

        new_worker.upsert(connection)
    }
}

#[derive(Insertable, Serialize, Deserialize, Debug)]
#[diesel(treat_none_as_null = true)]
#[diesel(table_name = workers)]
pub struct NewWorker {
    pub key: String,
    pub name: String,
    pub address: String,
    pub status: Option<String>,
    pub last_ping: NaiveDateTime,
    pub online: bool,
}

impl NewWorker {
    pub fn upsert(&self, connection: &mut SqliteConnection) -> Result<Worker> {
        let result = diesel::insert_into(workers::table)
            .values(self)
            .on_conflict(workers::key)
            .do_update()
            .set((
                workers::key.eq(excluded(workers::key)),
                workers::name.eq(&self.name),
                workers::address.eq(&self.address),
                workers::status.eq(&self.status),
                workers::last_ping.eq(&self.last_ping),
                workers::online.eq(&self.online),
            ))
            .returning(Worker::as_select())
            .get_result::<Worker>(connection)?;

        Ok(result)
    }
}
