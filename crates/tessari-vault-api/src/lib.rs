//! The HTTP API: the route table, the caller taken from a session, and the error shape every refusal answers with.

mod caller;
mod error;
mod handlers;
mod key_routes;
mod routes;
mod state;

pub use error::ApiError;
pub use routes::{Route, router, routes};
pub use state::AppState;
