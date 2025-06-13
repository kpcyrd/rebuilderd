use crate::code_migrations::compress_logs::CompressLogsMigration;
use crate::diesel::ExpressionMethods;
use diesel::migration::{Migration, MigrationVersion, Result};
use diesel::sqlite::Sqlite;
use diesel::{Connection, RunQueryDsl, SqliteConnection};

pub trait CodeMigration {
    fn prepare(
        &self,
        _connection: &mut SqliteConnection,
        _migration: &dyn Migration<Sqlite>,
    ) -> Result<()> {
        Ok(())
    }

    fn pre_up(
        &self,
        _connection: &mut SqliteConnection,
        _migration: &dyn Migration<Sqlite>,
    ) -> Result<()> {
        Ok(())
    }

    fn pre_down(
        &self,
        _connection: &mut SqliteConnection,
        _migration: &dyn Migration<Sqlite>,
    ) -> Result<()> {
        Ok(())
    }

    fn post_up(
        &self,
        _connection: &mut SqliteConnection,
        _migration: &dyn Migration<Sqlite>,
    ) -> Result<()> {
        Ok(())
    }

    fn post_down(
        &self,
        _connection: &mut SqliteConnection,
        _migration: &dyn Migration<Sqlite>,
    ) -> Result<()> {
        Ok(())
    }
}

struct UnitCodeMigration;
impl CodeMigration for UnitCodeMigration {}

// declare our understanding of what the diesel schema migration table looks like
diesel::table! {
    __diesel_schema_migrations (version) {
        version -> VarChar,
        run_on -> Timestamp,
    }
}

fn get_code_migration(migration: &dyn Migration<Sqlite>) -> Box<dyn CodeMigration> {
    match migration.name().to_string().as_str() {
        "2025-05-20-210543_compress-logs" => Box::new(CompressLogsMigration),
        _ => Box::new(UnitCodeMigration),
    }
}

pub fn run_code_backed_migration(
    connection: &mut SqliteConnection,
    migration: &dyn Migration<Sqlite>,
) -> Result<MigrationVersion<'static>> {
    let code_migration = get_code_migration(migration);

    let apply_migration = |conn: &mut SqliteConnection| -> Result<()> {
        code_migration.prepare(conn, migration)?;
        code_migration.pre_up(conn, migration)?;

        migration.run(conn)?;

        code_migration.post_up(conn, migration)?;

        diesel::insert_into(__diesel_schema_migrations::table)
            .values(__diesel_schema_migrations::version.eq(migration.name().version().as_owned()))
            .execute(conn)?;

        Ok(())
    };

    if migration.metadata().run_in_transaction() {
        connection.transaction(apply_migration)?;
    } else {
        apply_migration(connection)?;
    }

    Ok(migration.name().version().as_owned())
}
