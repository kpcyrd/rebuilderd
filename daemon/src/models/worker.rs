use crate::schema::*;
use chrono::prelude::*;
use diesel::prelude::*;
use rebuilderd_common::api;
use rebuilderd_common::errors::*;
use serde::{Deserialize, Serialize};
use std::net::IpAddr;

#[derive(Identifiable, Queryable, AsChangeset, Serialize, PartialEq, Eq, Debug)]
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

    pub fn new(key: String, addr: IpAddr, status: Option<String>) -> NewWorker {
        let now: DateTime<Utc> = Utc::now();

        NewWorker {
            key,
            address: addr.to_string(),
            status,
            last_ping: now.naive_utc(),
            online: true,
        }
    }
}
