use crate::schema::build_inputs;
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl, SqliteConnection, SqliteExpressionMethods};
use rebuilderd_common::errors::Error;

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

pub async fn get_build_input_friends(
    connection: &mut SqliteConnection,
    id: i32,
) -> Result<Vec<i32>, Error> {
    let results = build_input_friends(id).load::<i32>(connection)?;

    Ok(results)
}
