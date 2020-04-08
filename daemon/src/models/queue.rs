use crate::schema::*;
use rebuilderd_common::errors::*;
use rebuilderd_common::config::*;
use diesel::prelude::*;
use chrono::prelude::*;
use chrono::Duration;
use serde::{Serialize, Deserialize};
use rebuilderd_common::api::QueueItem;
use crate::models::Package;

#[derive(Identifiable, Queryable, AsChangeset, Serialize, PartialEq, Debug)]
#[table_name="queue"]
pub struct Queued {
    pub id: i32,
    pub package_id: i32,
    pub version: String,
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

    /*
    pub fn get(my_key: &str, connection: &SqliteConnection) -> Result<Option<Worker>> {
        use crate::schema::workers::dsl::*;
        let worker = workers.filter(key.eq(my_key))
            .first::<Worker>(connection)
            .optional()?;
        Ok(worker)
    }
    */

    pub fn pop_next(my_worker_id: i32, connection: &SqliteConnection) -> Result<Option<QueueItem>> {
        use crate::schema::queue::dsl::*;
        let item = queue
            .filter(worker_id.is_null())
            .order_by((queued_at, id))
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

    pub fn free_stale_jobs(connection: &SqliteConnection) -> Result<()> {
        use crate::schema::queue::columns::*;

        let now = Utc::now().naive_utc();
        let deadline = now - Duration::seconds(PING_DEADLINE);

        diesel::update(queue::table.filter(last_ping.lt(deadline)))
            .set(worker_id.eq(Option::<i32>::None))
            .execute(connection)?;

        Ok(())
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
            .order_by((queued_at, id)));

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

    pub fn queue_batch(pkgs: &[(i32, String)], connection: &SqliteConnection) -> Result<()> {
        let pkgs = pkgs.iter()
            .map(|(id, version)| NewQueued::new(*id, version.to_string()))
            .collect::<Vec<_>>();

        diesel::insert_into(queue::table)
            .values(pkgs)
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

    pub fn into_api_item(self, connection: &SqliteConnection) -> Result<QueueItem> {
        let pkg = Package::get_id(self.package_id, connection)?;

        Ok(QueueItem {
            id: self.id,
            package: pkg.into_api_item()?,
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
    pub package_id: i32,
    pub version: String,
    pub queued_at: NaiveDateTime,
    /*
    pub worker_id: Option<i32>,
    pub started_at: Option<NaiveDateTime>,
    pub last_ping: Option<NaiveDateTime>,
    */
}

impl NewQueued {
    pub fn new(package_id: i32, version: String) -> NewQueued {
        let now: DateTime<Utc> = Utc::now();
        NewQueued {
            package_id,
            version,
            queued_at: now.naive_utc(),
            /*
            worker_id: None,
            started_at: None,
            last_ping: None,
            */
        }
    }

    pub fn insert(&self, connection: &SqliteConnection) -> Result<()> {
        diesel::insert_into(queue::table)
            .values(self)
            .execute(connection)?;
        Ok(())
    }
}
