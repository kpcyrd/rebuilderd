use crate::fixtures::server::IsolatedServer;
use chrono::{Duration, NaiveDateTime, Utc};
use diesel::prelude::*;
use rebuilderd::schema::{queue, workers};

pub fn seconds_ago(seconds: i64) -> NaiveDateTime {
    Utc::now().naive_utc() - Duration::seconds(seconds)
}

pub fn set_worker_last_ping(isolated_server: &IsolatedServer, last_ping: NaiveDateTime) {
    let mut connection = isolated_server.connection();

    diesel::update(workers::table)
        .set(workers::last_ping.eq(last_ping))
        .execute(connection.as_mut())
        .unwrap();
}

pub fn set_job_last_ping(isolated_server: &IsolatedServer, last_ping: NaiveDateTime) {
    let mut connection = isolated_server.connection();

    diesel::update(queue::table.filter(queue::last_ping.is_not_null()))
        .set(queue::last_ping.eq(last_ping))
        .execute(connection.as_mut())
        .unwrap();
}
