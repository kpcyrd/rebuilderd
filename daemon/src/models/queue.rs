use crate::models::{BinaryPackage, BuildInput, SourcePackage};
use crate::schema::*;
use chrono::prelude::*;
use chrono::Duration;
use diesel::prelude::*;
use diesel::upsert::excluded;
use rebuilderd_common::api::v0::{PkgArtifact, QueueItem};
use rebuilderd_common::config::*;
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
    pub fn get_id(my_id: i32, connection: &mut SqliteConnection) -> Result<Queued> {
        use crate::schema::queue::dsl::*;
        let item = queue.filter(id.eq(my_id)).first::<Queued>(connection)?;
        Ok(item)
    }

    pub fn get(build_input: i32, connection: &mut SqliteConnection) -> Result<Option<Queued>> {
        use crate::schema::queue::dsl::*;
        let job = queue
            .filter(build_input_id.eq(build_input))
            .first::<Queued>(connection)
            .optional()?;
        Ok(job)
    }

    pub fn ping_job(&mut self, connection: &mut SqliteConnection) -> Result<()> {
        let now: DateTime<Utc> = Utc::now();
        self.last_ping = Some(now.naive_utc());
        self.update(connection)
    }

    pub fn delete(&self, connection: &mut SqliteConnection) -> Result<()> {
        use crate::schema::queue::columns::*;
        diesel::delete(queue::table.filter(id.eq(self.id))).execute(connection)?;
        Ok(())
    }

    pub fn update(&self, connection: &mut SqliteConnection) -> Result<()> {
        use crate::schema::queue::columns::*;
        diesel::update(queue::table.filter(id.eq(self.id)))
            .set(self)
            .execute(connection)?;
        Ok(())
    }

    pub fn drop_for_source_packages(
        source_package_ids: &[i32],
        connection: &mut SqliteConnection,
    ) -> Result<()> {
        let ids = queue::table
            .inner_join(build_inputs::table)
            .select(queue::id)
            .filter(build_inputs::source_package_id.eq_any(source_package_ids))
            .load::<i32>(connection)?;

        diesel::delete(queue::table.filter(queue::id.eq_any(ids))).execute(connection)?;
        Ok(())
    }

    pub fn free_stale_jobs(connection: &mut SqliteConnection) -> Result<()> {
        let now = Utc::now().naive_utc();
        let deadline = now - Duration::seconds(PING_DEADLINE);

        diesel::update(queue::table.filter(queue::last_ping.lt(deadline)))
            .set((
                queue::worker.eq(Option::<i32>::None),
                queue::started_at.eq(Option::<NaiveDateTime>::None),
                queue::last_ping.eq(Option::<NaiveDateTime>::None),
            ))
            .execute(connection)?;

        Ok(())
    }

    pub fn into_api_item(self, connection: &mut SqliteConnection) -> Result<QueueItem> {
        let build_input = build_inputs::table
            .filter(build_inputs::id.eq(self.build_input_id))
            .get_result::<BuildInput>(connection)?;

        let source_package = SourcePackage::get_id(build_input.source_package_id, connection)?;
        let binary_packages = binary_packages::table
            .filter(binary_packages::source_package_id.eq(source_package.id))
            .load::<BinaryPackage>(connection)?;

        let version = source_package.version.clone();
        let artifacts = binary_packages
            .iter()
            .map(|b| PkgArtifact {
                name: b.name.clone(),
                version: b.version.clone(),
                url: b.artifact_url.clone(),
            })
            .collect();

        let pkgbase = source_package.into_api_item(
            build_input.architecture,
            Some(build_input.url),
            artifacts,
        )?;

        Ok(QueueItem {
            id: self.id,
            pkgbase,
            version,
            queued_at: self.queued_at,
            worker_id: self.worker,
            started_at: self.started_at,
            last_ping: self.last_ping,
        })
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
    pub fn new(build_input_id: i32, priority: i32) -> NewQueued {
        let now: DateTime<Utc> = Utc::now();
        NewQueued {
            build_input_id,
            priority,
            queued_at: now.naive_utc(),
        }
    }

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
