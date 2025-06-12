use diesel::helper_types::{Asc, Desc};
use diesel::query_dsl::methods::OrderDsl;
use diesel::{Expression, ExpressionMethods, QueryDsl};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Page {
    pub limit: Option<i32>,
    pub before: Option<i32>,
    pub after: Option<i32>,
    pub sort: Option<String>,
    pub direction: Option<SortDirection>,
}

#[derive(Debug, PartialEq, Eq, Clone, Deserialize)]
pub enum SortDirection {
    Ascending,
    Descending,
}

enum SortOrder<Expr> {
    Asc(Asc<Expr>),
    Desc(Desc<Expr>),
}

impl<Expr> SortOrder<Expr>
where
    Expr: Expression,
{
    fn new_asc<E: Expression + ExpressionMethods>(e: E) -> SortOrder<E> {
        SortOrder::Asc(e.asc())
    }
    fn new_desc<E: Expression + ExpressionMethods>(e: E) -> SortOrder<E> {
        SortOrder::Desc(e.desc())
    }

    fn apply<Q, E>(self, sql: Q) -> Q
    where
        Q: QueryDsl + OrderDsl<Expr>,
    {
        match self {
            Self::Asc(e) => sql.order(e),
            Self::Desc(e) => sql.order(e),
        }
    }
}
