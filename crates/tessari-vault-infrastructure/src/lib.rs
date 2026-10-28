//! Adapters with no domain logic: configuration, the TessariDB connection and randomness.

pub mod config;
mod error;
pub mod random;
pub mod tessaridb;

pub use error::{Error, Result};
