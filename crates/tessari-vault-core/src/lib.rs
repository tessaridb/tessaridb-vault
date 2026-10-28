//! The domain of TessariDB Vault: companies, members, groups, records and the access rules over them. No I/O.

pub mod access;
mod error;
pub mod totp;
pub mod webauthn;

pub use error::{Error, Result};
