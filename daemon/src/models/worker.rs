use crate::schema::*;
use chrono::Duration;
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

    pub fn mark_stale_offline(
        connection: &mut SqliteConnection,
        offline_deadline: Duration,
    ) -> Result<usize> {
        let deadline = Utc::now().naive_utc() - offline_deadline;

        let updated = diesel::update(
            workers::table
                .filter(workers::online.eq(true))
                .filter(workers::last_ping.lt(deadline)),
        )
        .set((
            workers::online.eq(false),
            workers::status.eq(None as Option<String>),
        ))
        .execute(connection)?;

        Ok(updated)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use rebuilderd_common::config::PING_DEADLINE;

    fn deadline() -> Duration {
        Duration::seconds(PING_DEADLINE)
    }

    fn insert_worker(
        connection: &mut SqliteConnection,
        key: &str,
        last_ping: NaiveDateTime,
    ) -> Worker {
        NewWorker {
            key: key.to_string(),
            name: key.to_string(),
            address: "127.0.0.1".to_string(),
            status: Some("working hard".to_string()),
            last_ping,
            online: true,
        }
        .upsert(connection)
        .unwrap()
    }

    fn load_worker(connection: &mut SqliteConnection, id: i32) -> Worker {
        workers::table
            .find(id)
            .select(Worker::as_select())
            .get_result(connection)
            .unwrap()
    }

    #[test]
    fn mark_stale_offline_only_affects_workers_past_the_deadline() {
        let mut connection = db::setup(":memory:").unwrap();
        let now = Utc::now().naive_utc();

        let fresh = insert_worker(&mut connection, "fresh", now);
        let stale = insert_worker(
            &mut connection,
            "stale",
            now - Duration::seconds(PING_DEADLINE + 60),
        );

        assert_eq!(1, Worker::mark_stale_offline(&mut connection, deadline()).unwrap());

        let fresh = load_worker(&mut connection, fresh.id);
        assert!(fresh.online);
        assert_eq!(Some("working hard".to_string()), fresh.status);

        let stale = load_worker(&mut connection, stale.id);
        assert!(!stale.online);
        assert_eq!(None, stale.status);

        // workers that are already offline are left alone
        assert_eq!(0, Worker::mark_stale_offline(&mut connection, deadline()).unwrap());
    }

    #[test]
    fn ping_brings_stale_worker_back_online() {
        let mut connection = db::setup(":memory:").unwrap();
        let stale = insert_worker(
            &mut connection,
            "stale",
            Utc::now().naive_utc() - Duration::seconds(PING_DEADLINE + 60),
        );

        Worker::mark_stale_offline(&mut connection).unwrap();
        let worker = Worker::get_and_refresh(&stale.key, &mut connection).unwrap();

        assert!(worker.online);
    }
}
