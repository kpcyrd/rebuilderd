use crate::schema::*;
use rebuilderd_common::errors::*;
use rebuilderd_common::config::*;
use diesel::prelude::*;
use chrono::prelude::*;
use chrono::Duration;
use serde::{Serialize, Deserialize};
use rebuilderd_common::api::QueueItem;
use crate::models::PkgBase;

#[derive(Identifiable, Queryable, AsChangeset, Serialize, PartialEq, Debug)]
#[table_name="queue"]
pub struct Queued {
    pub id: i32,
    pub pkgbase_id: i32,
    pub version: String,
    pub required_backend: String,
    pub priority: i32,
    pub queued_at: NaiveDateTime,
    pub worker_id: Option<i32>,
    pub started_at: Option<NaiveDateTime>,
    pub last_ping: Option<NaiveDateTime>,
}

impl Queued {
    pub fn get_id(my_id: i32, connection: &SqliteConnection) -> Result<Queued> {
        use crate::schema::queue::dsl::*;
        let item = queue
            .filter(id.eq(my_id))
            .first::<Queued>(connection)?;
        Ok(item)
    }

    pub fn get(pkg: i32, my_version: &str, connection: &SqliteConnection) -> Result<Option<Queued>> {
        use crate::schema::queue::dsl::*;
        let job = queue
            .filter(pkgbase_id.eq(pkg))
            .filter(version.eq(my_version))
            .first::<Queued>(connection)
            .optional()?;
        Ok(job)
    }

    pub fn pop_next(my_worker_id: i32, supported_backends: &[String], connection: &SqliteConnection) -> Result<Option<QueueItem>> {
        use crate::schema::queue::dsl::*;
        let mut query = queue
            .filter(worker_id.is_null())
            .into_boxed();

        if !supported_backends.is_empty() {
            query = query
                .filter(required_backend.eq_any(supported_backends));
        }

        let item = query
            .order_by((priority, queued_at, id))
            .first::<Queued>(connection)
            .optional()?;
        if let Some(mut item) = item {
            let now: DateTime<Utc> = Utc::now();

            item.worker_id = Some(my_worker_id);
            item.started_at = Some(now.naive_utc());
            item.last_ping = Some(now.naive_utc());
            item.update(connection)?;

            Ok(Some(item.into_api_item(connection)?))
        } else {
            Ok(None)
        }
    }

    pub fn ping_job(&mut self, connection: &SqliteConnection) -> Result<()> {
        let now: DateTime<Utc> = Utc::now();
        self.last_ping = Some(now.naive_utc());
        self.update(connection)
    }

    pub fn delete(&self, connection: &SqliteConnection) -> Result<()> {
        use crate::schema::queue::columns::*;
        diesel::delete(queue::table
            .filter(id.eq(self.id))
        ).execute(connection)?;
        Ok(())
    }

    pub fn list(limit: Option<i64>, connection: &SqliteConnection) -> Result<Vec<Queued>> {
        use crate::schema::queue::dsl::*;

        let query = Box::new(queue
            .order_by((priority, queued_at, id)));

        let results = if let Some(limit) = limit {
            query
                .limit(limit)
                .load::<Queued>(connection)?
        } else {
            query
                .load::<Queued>(connection)?
        };

        Ok(results)
    }

    pub fn update(&self, connection: &SqliteConnection) -> Result<()> {
        use crate::schema::queue::columns::*;
        diesel::update(queue::table.filter(id.eq(self.id)))
            .set(self)
            .execute(connection)?;
        Ok(())
    }

    pub fn queue_batch(pkgbases: &[(i32, String)], required_backend: String, priority: i32, connection: &SqliteConnection) -> Result<()> {
        let pkgbases = pkgbases.iter()
            .map(|(id, version)| NewQueued::new(*id, version.to_string(), required_backend.clone(), priority))
            .collect::<Vec<_>>();

        diesel::insert_into(queue::table)
            .values(pkgbases)
            // TODO: not supported by diesel yet
            // .on_conflict_do_nothing()
            .execute(connection)?;

        Ok(())
    }

    pub fn drop_for_pkgbases(pkgbases: &[i32], connection: &SqliteConnection) -> Result<()> {
        diesel::delete(queue::table.filter(queue::pkgbase_id.eq_any(pkgbases)))
            .execute(connection)?;
        Ok(())
    }

    pub fn requeue(&self, connection: &SqliteConnection) -> Result<()> {
        diesel::update(queue::table)
            .filter(queue::id.eq(self.id))
            .set((
                queue::worker_id.eq(Option::<i32>::None),
                queue::started_at.eq(Option::<NaiveDateTime>::None),
                queue::last_ping.eq(Option::<NaiveDateTime>::None),
            ))
            .execute(connection)?;

        Ok(())
    }

    pub fn free_stale_jobs(connection: &SqliteConnection) -> Result<()> {
        let now = Utc::now().naive_utc();
        let deadline = now - Duration::seconds(PING_DEADLINE);

        diesel::update(queue::table.filter(queue::last_ping.lt(deadline)))
            .set((
                queue::worker_id.eq(Option::<i32>::None),
                queue::started_at.eq(Option::<NaiveDateTime>::None),
                queue::last_ping.eq(Option::<NaiveDateTime>::None),
            ))
            .execute(connection)?;

        Ok(())
    }

    pub fn into_api_item(self, connection: &SqliteConnection) -> Result<QueueItem> {
        let pkgbase = PkgBase::get_id(self.pkgbase_id, connection)?;

        Ok(QueueItem {
            id: self.id,
            pkgbase: pkgbase.into_api_item()?,
            version: self.version,
            queued_at: self.queued_at,
            worker_id: self.worker_id,
            started_at: self.started_at,
            last_ping: self.last_ping,
        })
    }
}

#[derive(Insertable, Serialize, Deserialize, Debug)]
#[table_name="queue"]
pub struct NewQueued {
    pub pkgbase_id: i32,
    pub version: String,
    pub required_backend: String,
    pub priority: i32,
    pub queued_at: NaiveDateTime,
}

impl NewQueued {
    pub fn new(pkgbase_id: i32, version: String, required_backend: String, priority: i32) -> NewQueued {
        let now: DateTime<Utc> = Utc::now();
        NewQueued {
            pkgbase_id,
            version,
            required_backend,
            priority,
            queued_at: now.naive_utc(),
        }
    }

    pub fn insert(&self, connection: &SqliteConnection) -> Result<()> {
        // TODO: on conflict do nothing after it landed in diesel sqlite
        if Queued::get(self.pkgbase_id, &self.version, connection)?.is_none() {
            diesel::insert_into(queue::table)
                .values(self)
                .execute(connection)?;
        }
        Ok(())
    }
}
