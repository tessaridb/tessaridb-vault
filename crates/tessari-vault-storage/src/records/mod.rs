//! Records: their rows, their revisions, and the sealed bodies kept in each company's records vault.

mod entity;
mod repository;
mod service;
mod tessaridb;
mod vault;

pub use entity::{OpenedRecord, Record, Sealed, SealedError};
pub use service::RecordService;
pub(crate) use tessaridb::{AUTHORS_RECORD, TessaridbRecordRepository};
pub(crate) use vault::RecordVaults;
