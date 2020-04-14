use crate::schema::*;
use rebuilderd_common::api;
use rebuilderd_common::config::*;
use rebuilderd_common::errors::*;
use diesel::prelude::*;
use chrono::prelude::*;
use chrono::Duration;
use serde::{Serialize, Deserialize};
use std::net::IpAddr;

#[derive(Identifiable, Queryable, AsChangeset, Serialize, PartialEq, Debug)]
#[table_name="workers"]
pub struct Worker {
    pub id: i32,
    pub key: String,
    pub addr: String,
    pub status: Option<String>,
    pub last_ping: NaiveDateTime,
    pub online: bool,
}

impl Worker {
    pub fn get(my_key: &str, connection: &SqliteConnection) -> Result<Option<Worker>> {
        use crate::schema::workers::dsl::*;
        let worker = workers.filter(key.eq(my_key))
            .first::<Worker>(connection)
            .optional()?;
        Ok(worker)
    }

    pub fn list(connection: &SqliteConnection) -> Result<Vec<Worker>> {
        use crate::schema::workers::dsl::*;
        let results = workers.filter(online.eq(true))
            .load::<Worker>(connection)?;
        Ok(results)
    }

    pub fn mark_stale_workers_offline(connection: &SqliteConnection) -> Result<()> {
        use crate::schema::workers::columns::*;

        let now = Utc::now().naive_utc();
        let deadline = now - Duration::seconds(PING_DEADLINE);

        diesel::update(workers::table.filter(last_ping.lt(deadline)))
            .set((
                online.eq(false),
                status.eq(None as Option::<String>),
            ))
            .execute(connection)?;

        Ok(())
    }

    pub fn bump_last_ping(&mut self) {
        let now = Utc::now().naive_utc();
        self.last_ping = now;
        self.online = true;
    }

    pub fn update(&self, connection: &SqliteConnection) -> Result<()> {
        use crate::schema::workers::columns::*;
        diesel::update(workers::table.filter(id.eq(self.id)))
            .set(self)
            .execute(connection)?;

        // workaround until we can have a model that can update to null at the same time
        if self.status.is_none() {
            diesel::update(workers::table.filter(id.eq(self.id)))
                .set(
                    status.eq(None as Option::<String>),
                )
                .execute(connection)?;
        }

        Ok(())
    }
}

impl From<Worker> for api::Worker {
    fn from(worker: Worker) -> api::Worker {
        api::Worker {
            key: worker.key,
            addr: worker.addr,
            status: worker.status,
            last_ping: worker.last_ping,
            online: worker.online,
        }
    }
}

#[derive(Insertable, Serialize, Deserialize, Debug)]
#[table_name="workers"]
pub struct NewWorker {
    pub key: String,
    pub addr: String,
    pub status: Option<String>,
    pub last_ping: NaiveDateTime,
    pub online: bool,
}

impl NewWorker {
    // we can refactor this into an upsert if we can make it return a Worker struct
    pub fn insert(&self, connection: &SqliteConnection) -> Result<()> {
        diesel::insert_into(workers::table)
            .values(self)
            .execute(connection)?;
        Ok(())
    }

    pub fn new(key: String, addr: IpAddr, status: Option<String>) -> NewWorker {
        let now: DateTime<Utc> = Utc::now();

        NewWorker {
            key,
            addr: addr.to_string(),
            status,
            last_ping: now.naive_utc(),
            online: true,
        }
    }
}
