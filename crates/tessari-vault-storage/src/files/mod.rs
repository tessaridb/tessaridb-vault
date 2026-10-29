//! Encrypted files attached to records: kept in the company's records vault when small, in a bucket when larger.

mod entity;
mod repository;
mod service;
mod tessaridb;

pub use entity::{FileMeta, Kept};
pub use service::FileService;
pub(crate) use tessaridb::{TessaridbFiles, bucket_path, part_size, vault_entry};
