//! Everything TessariDB Vault keeps, on TessariDB, reached only through domain services that decide before they
//! read or write.

pub mod access;
pub mod accounts;
mod answers;
pub mod companies;
pub mod enrolments;
mod error;
pub mod groups;
pub mod identities;
pub mod passkeys;
mod schema;
pub mod sessions;
pub mod signin;
mod storage;
pub mod throttle;

pub use error::{Conflict, Error, Result};
pub use schema::migrate;
pub use storage::{Custody, Storage};
