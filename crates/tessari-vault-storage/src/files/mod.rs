//! Encrypted files attached to records: kept in the company's records vault when small, in a bucket when larger.

mod entity;
mod repository;
mod s3;
mod service;
mod tessaridb;

pub use entity::{FileMeta, Kept};
pub(crate) use repository::FileRepository;
pub(crate) use s3::S3Files;
pub use service::FileService;
pub(crate) use tessaridb::{TessaridbFiles, bucket_path, part_size, vault_entry};
