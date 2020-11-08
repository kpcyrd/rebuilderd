pub use actix_web::web::{post, Json, Data, Query, Path, resource};
use rebuilderd_common::errors;
use std::fmt;

#[derive(Debug)]
pub struct Error {
    err: rebuilderd_common::errors::Error,
}

pub type Result<T> = ::std::result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, w: &mut fmt::Formatter) -> fmt::Result {
        self.err.fmt(w)
    }
}

impl actix_web::error::ResponseError for Error {
}

impl From<errors::Error> for Error {
    fn from(err: errors::Error) -> Error {
        Error { err }
    }
}
