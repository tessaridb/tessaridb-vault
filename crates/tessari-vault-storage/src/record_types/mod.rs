//! Record types a company defines: who may create records of each, and the record holding its sealed definition.

mod entity;
mod repository;
mod service;
mod tessaridb;

pub use entity::RecordTypeDefinition;
pub use service::RecordTypeService;
pub(crate) use tessaridb::{TessaridbRecordTypeRepository, role_words};
