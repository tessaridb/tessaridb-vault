//! Everything TessariDB Vault keeps, on TessariDB, reached only through domain services that decide before they
//! read or write.

pub mod access;
pub mod accounts;
mod answers;
pub mod audit;
pub mod chain;
pub mod companies;
pub mod enrolments;
mod error;
pub mod files;
pub mod group_keys;
pub mod groups;
pub mod identities;
pub mod keys;
pub mod passkeys;
pub mod record_types;
pub mod records;
pub mod recovery;
pub mod recovery_requests;
pub mod rotation;
mod schema;
pub mod sessions;
pub mod shares;
pub mod signin;
mod storage;
pub mod throttle;

pub use error::{Conflict, Error, Result};
pub use schema::migrate;
pub use storage::{Custody, Storage};
