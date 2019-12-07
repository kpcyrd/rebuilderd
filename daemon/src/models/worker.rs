use crate::schema::*;
use rebuilderd_common::api;
use rebuilderd_common::errors::*;
use diesel::prelude::*;
use chrono::prelude::*;
use serde::{Serialize, Deserialize};
use std::net::IpAddr;

#[derive(Identifiable, Queryable, AsChangeset, Serialize, PartialEq, Debug)]
#[table_name="workers"]
pub struct Worker {
    pub id: i32,
    pub key: String,
    // TODO: pub addr: IpAddr,
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

    pub fn update(&self, connection: &SqliteConnection) -> Result<()> {
        use crate::schema::workers::columns::*;
        diesel::update(workers::table.filter(id.eq(self.id)))
            .set(self)
            .execute(connection)?;
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
    pub fn insert(&self, connection: &SqliteConnection) -> Result<()> {
        if let Some(mut worker) = Worker::get(&self.key, connection)? {
            worker.status = self.status.clone();
            worker.last_ping = self.last_ping;
            worker.online = true;
            return worker.update(connection);
        }

        diesel::insert_into(workers::table)
            .values(self)
            .execute(connection)?;
        Ok(())
    }

    pub fn new(query: api::WorkQuery, addr: IpAddr, status: Option<String>) -> NewWorker {
        let now: DateTime<Utc> = Utc::now();

        NewWorker {
            key: query.key,
            addr: addr.to_string(),
            status,
            last_ping: now.naive_utc(),
            online: true,
        }
    }
}
