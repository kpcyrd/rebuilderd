use diesel::migration::Result;
use diesel::migration::{Migration, MigrationVersion};
use diesel::sqlite::Sqlite;
use diesel::{Connection, SqliteConnection};
use diesel_migrations::MigrationHarness;

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

fn get_code_migration(migration: &dyn Migration<Sqlite>) -> Box<dyn CodeMigration> {
    match migration.name().to_string().as_str() {
        _ => Box::new(UnitCodeMigration)
    }
}

pub fn run_code_backed_migration(
    connection: &mut SqliteConnection,
    migration: &dyn Migration<Sqlite>,
) -> Result<MigrationVersion<'static>> {
    let code_migration = get_code_migration(migration);

    code_migration.prepare(connection, migration)?;

    if migration.metadata().run_in_transaction() {
        connection.transaction(|c| code_migration.pre_up(c, migration))?;
    } else {
        code_migration.pre_up(connection, migration)?;
    }

    let version = connection.run_migration(migration)?;

    if migration.metadata().run_in_transaction() {
        connection.transaction(|c| code_migration.post_up(c, migration))?;
    } else {
        code_migration.post_up(connection, migration)?;
    }

    Ok(version)
}
