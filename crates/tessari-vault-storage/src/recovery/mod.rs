//! Organisation recovery (C7): the company's recovery key, an X25519 key whose private half each owner's browser
//! wraps to the owners' own key sets; the server keeps the public half and the wraps, never the key.

mod entity;
mod repository;
mod service;
mod tessaridb;

pub use entity::RecoveryKey;
pub use service::RecoveryService;
pub(crate) use tessaridb::TessaridbRecoveryRepository;
