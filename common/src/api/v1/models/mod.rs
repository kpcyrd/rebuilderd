mod build;
mod miscellaneous;
mod package;
mod queue;
mod worker;

pub use build::*;
pub use miscellaneous::*;
pub use package::*;
pub use queue::*;
use serde::{Deserialize, Serialize};
pub use worker::*;

#[derive(Debug, Serialize, Deserialize)]
pub struct ResultPage<T> {
    pub total: i64,
    pub records: Vec<T>,
}
