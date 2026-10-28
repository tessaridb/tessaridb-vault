//! Records: their rows, their revisions, and the sealed bodies kept in each company's records vault.

mod entity;
mod opened;
mod repository;
mod service;
mod tessaridb;
mod vault;

pub use entity::{OpenedRecord, Record, Revision, Sealed, SealedError};
pub use service::RecordService;
pub(crate) use tessaridb::{COMPANYS_RECORD, TessaridbRecordRepository};
pub(crate) use vault::RecordVaults;
