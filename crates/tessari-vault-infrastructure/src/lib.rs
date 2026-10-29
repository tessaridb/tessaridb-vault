//! Adapters with no domain logic: configuration, key custody, the TessariDB connection and randomness.

pub mod config;
pub mod custody;
mod error;
pub mod hardening;
pub mod health;
pub mod oidc;
pub mod random;
pub mod tessaridb;

pub use error::{Error, Result};
