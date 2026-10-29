//! The key routes of the table in `routes`: members' key sets, the vouching chain, group keys, the recovery key and
//! recovery requests.

use axum::http::Method;
use axum::routing::{get, post, put};

use crate::handlers::{group_keys, keys, recovery, recovery_requests};
use crate::routes::{Route, route};

/// The key routes, which `routes::routes` serves with the rest.
pub(crate) fn routes() -> Vec<Route> {
    vec![
        route(
            Method::POST,
            "/api/companies/{company}/recovery-key",
            post(recovery::keep),
        ),
        route(
            Method::GET,
            "/api/companies/{company}/recovery-key",
            get(recovery::read),
        ),
        route(
            Method::GET,
            "/api/companies/{company}/recovery-key/mine",
            get(recovery::mine),
        ),
        route(
            Method::PUT,
            "/api/companies/{company}/keys/me/escrow",
            put(recovery::escrow),
        ),
        route(
            Method::GET,
            "/api/companies/{company}/keys/{account}/escrow",
            get(recovery::escrow_of),
        ),
        route(
            Method::GET,
            "/api/companies/{company}/notices/me",
            get(recovery::notices),
        ),
        route(
            Method::POST,
            "/api/companies/{company}/recovery-requests",
            post(recovery_requests::ask),
        ),
        route(
            Method::GET,
            "/api/companies/{company}/recovery-requests",
            get(recovery_requests::list),
        ),
        route(
            Method::POST,
            "/api/companies/{company}/recovery-requests/{request}/handoff",
            post(recovery_requests::hand_over),
        ),
        route(
            Method::POST,
            "/api/companies/{company}/recovery-requests/{request}/redeem",
            post(recovery_requests::redeem),
        ),
        route(
            Method::POST,
            "/api/companies/{company}/keys/me/rewrap",
            post(recovery_requests::rewrap),
        ),
        route(
            Method::PUT,
            "/api/companies/{company}/groups/{group}/key",
            put(group_keys::keep),
        ),
        route(
            Method::GET,
            "/api/companies/{company}/groups/{group}/key",
            get(group_keys::read),
        ),
        route(
            Method::GET,
            "/api/companies/{company}/groups/{group}/key/pending",
            get(group_keys::pending),
        ),
        route(
            Method::POST,
            "/api/companies/{company}/groups/{group}/key/wraps",
            post(group_keys::add_wraps),
        ),
        route(
            Method::PUT,
            "/api/companies/{company}/keys/me",
            put(keys::keep),
        ),
        route(
            Method::GET,
            "/api/companies/{company}/keys/me",
            get(keys::mine),
        ),
        route(
            Method::GET,
            "/api/companies/{company}/keys",
            get(keys::directory),
        ),
        route(
            Method::GET,
            "/api/companies/{company}/statements",
            get(keys::chain),
        ),
        route(
            Method::POST,
            "/api/companies/{company}/statements",
            post(keys::append),
        ),
    ]
}
