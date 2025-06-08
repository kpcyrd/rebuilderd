use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Page {
    pub limit: Option<i32>,
    pub before: Option<i32>,
    pub after: Option<i32>,
    pub sort: Option<String>,
    pub direction: Option<SortDirection>,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
pub enum SortDirection {
    Ascending,
    Descending,
}
