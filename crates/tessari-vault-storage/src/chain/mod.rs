//! The vouching chain (KH-L3): signed statements a company's browsers verify from the owner key pinned at its
//! genesis. The server keeps them in order and decides who may append; what they say is for the browsers to check.

mod entity;
mod repository;
mod service;
mod tessaridb;

pub use entity::{Chain, Statement};
pub use service::ChainService;
pub(crate) use tessaridb::TessaridbChainRepository;
