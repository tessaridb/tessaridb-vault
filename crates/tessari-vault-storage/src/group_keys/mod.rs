//! Group keys (KH-L1): a company group, department or team holds an X25519 key whose private half the admins'
//! browsers wrap to each member inside the group's reach; the server keeps the wraps and never the key.

mod entity;
mod reach;
mod repository;
mod service;
mod tessaridb;

pub(crate) use entity::Revocation;
pub use entity::{GroupKey, MemberWrap, NewGroupKey};
pub use service::GroupKeyService;
pub(crate) use tessaridb::{TessaridbGroupKeyRepository, recipient_wraps, recipients};
