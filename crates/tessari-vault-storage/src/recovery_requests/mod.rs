//! Recovery requests (C7): a member who lost the unlock password asks with a one-time handoff key, an owner's browser
//! hands the member's escrowed keys over wrapped to it, and the member redeems the handoff once and wraps their keys
//! under a new unlock key. Each request is single-use and closes after 72 hours.

mod entity;
mod repository;
mod service;
mod tessaridb;

pub use entity::OpenRequest;
pub use service::RecoveryRequestService;
pub(crate) use tessaridb::TessaridbRecoveryRequestRepository;
