use crate::schema::*;
use chrono::prelude::*;
use diesel::prelude::*;
use diesel::upsert::excluded;
use rebuilderd_common::api;
use rebuilderd_common::errors::*;
use serde::{Deserialize, Serialize};
use std::net::IpAddr;

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
    pub fn get(my_key: &str, connection: &mut SqliteConnection) -> Result<Option<Worker>> {
        use crate::schema::workers::dsl::*;
        let worker = workers
            .filter(key.eq(my_key))
            .first::<Worker>(connection)
            .optional()?;
        Ok(worker)
    }

    pub fn bump_last_ping(&mut self, addr: &IpAddr) {
        self.address = addr.to_string();
        let now = Utc::now().naive_utc();
        self.last_ping = now;
        self.online = true;
    }

    pub fn update(&self, connection: &mut SqliteConnection) -> Result<()> {
        use crate::schema::workers::columns::*;
        diesel::update(workers::table.filter(id.eq(self.id)))
            .set(self)
            .execute(connection)?;

        // workaround until we can have a model that can update to null at the same time
        if self.status.is_none() {
            diesel::update(workers::table.filter(id.eq(self.id)))
                .set(status.eq(None as Option<String>))
                .execute(connection)?;
        }

        Ok(())
    }

    pub fn get_and_refresh(key: &str, connection: &mut SqliteConnection) -> Result<Worker> {
        let worker = diesel::update(workers::table.filter(workers::key.eq(key)))
            .set((
                workers::last_ping.eq(Utc::now().naive_utc()),
                workers::online.eq(true),
            ))
            .returning(Worker::as_select())
            .get_result(connection)?;

        Ok(worker)
    }
}

impl From<Worker> for api::v0::Worker {
    fn from(worker: Worker) -> api::v0::Worker {
        api::v0::Worker {
            key: worker.key,
            addr: worker.address,
            status: worker.status,
            last_ping: worker.last_ping,
            online: worker.online,
        }
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
    // we can refactor this into an upsert if we can make it return a Worker struct
    pub fn insert(&self, connection: &mut SqliteConnection) -> Result<()> {
        diesel::insert_into(workers::table)
            .values(self)
            .execute(connection)?;
        Ok(())
    }

    pub fn new(
        key: String,
        name: Option<String>,
        addr: IpAddr,
        status: Option<String>,
    ) -> NewWorker {
        let now: DateTime<Utc> = Utc::now();

        NewWorker {
            key,
            name: name.unwrap_or("".to_string()),
            address: addr.to_string(),
            status,
            last_ping: now.naive_utc(),
            online: true,
        }
    }

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
