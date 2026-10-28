//! Departments and teams, and who is in them.

mod entity;
mod repository;
mod service;
mod tessaridb;

pub use entity::Group;
pub use service::GroupService;
pub(crate) use tessaridb::TessaridbGroupRepository;
