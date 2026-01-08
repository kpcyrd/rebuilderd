use crate::schema::{build_inputs, queue};
use chrono::NaiveDateTime;
use diesel::{
    ExpressionMethods, QueryDsl, QueryResult, RunQueryDsl, SqliteConnection,
    SqliteExpressionMethods, delete, update,
};

use aliases::*;

mod aliases {
    diesel::alias!(crate::schema::build_inputs as b1: BuildInputsAlias);
}

#[diesel::dsl::auto_type]
pub fn build_input_friends(id: i32) -> _ {
    build_inputs::table
        .select(build_inputs::id)
        .filter(
            build_inputs::url.eq_any(
                b1.filter(b1.field(build_inputs::id).is(id))
                    .select(b1.field(build_inputs::url)),
            ),
        )
        .filter(
            build_inputs::backend.eq_any(
                b1.filter(b1.field(build_inputs::id).is(id))
                    .select(b1.field(build_inputs::backend)),
            ),
        )
        .filter(
            build_inputs::architecture.eq_any(
                b1.filter(b1.field(build_inputs::id).is(id))
                    .select(b1.field(build_inputs::architecture)),
            ),
        )
}

pub fn get_build_input_friends(
    connection: &mut SqliteConnection,
    id: i32,
) -> QueryResult<Vec<i32>> {
    build_input_friends(id).load::<i32>(connection)
}

/// Set `next_retry` of the build_input to NULL, and remove any related item
/// from the build queue
pub fn mark_build_input_friends_as_non_retriable(
    connection: &mut SqliteConnection,
    id: i32,
) -> QueryResult<()> {
    let friends = get_build_input_friends(connection, id)?;

    // null out the next retry to mark the package and its friends as non-retried
    update(build_inputs::table)
        .filter(build_inputs::id.eq_any(&friends))
        .set(build_inputs::next_retry.eq(None::<NaiveDateTime>))
        .execute(connection)?;

    // drop any enqueued jobs for the build input and its friends
    delete(queue::table)
        .filter(queue::build_input_id.eq_any(&friends))
        .execute(connection)?;

    Ok(())
}
