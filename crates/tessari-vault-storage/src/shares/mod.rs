//! Shares: a record shared with a group or one member, each through a wrap of the record key to a key the target
//! holds, and the reach every record and file call decides on.

mod entity;
mod reach;
mod repository;
mod service;
mod tessaridb;

pub use entity::{Access, ListedShare, ListedShares, NewShare, ShareError, ShareTarget};
pub(crate) use reach::{Reach, Reached};
pub use service::ShareService;
pub(crate) use tessaridb::{TessaridbShareRepository, retiring};
