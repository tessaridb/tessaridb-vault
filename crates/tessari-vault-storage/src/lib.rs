//! Everything TessariDB Vault keeps, on TessariDB, reached only through domain services that decide before they
//! read or write.

pub mod access;
pub mod accounts;
mod answers;
pub mod companies;
mod error;
pub mod groups;
mod schema;
pub mod sessions;
mod storage;
pub mod throttle;

pub use error::{Conflict, Error, Result};
pub use schema::migrate;
pub use storage::Storage;
