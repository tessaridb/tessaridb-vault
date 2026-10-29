//! The route table. The router is built from it and nothing else, so it is also the source set of the
//! enforcement-point coverage matrix: a route added anywhere else would not be served.

use std::collections::BTreeMap;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::http::Method;
use axum::routing::{MethodRouter, delete, get, patch, post, put};

use crate::handlers::{
    audit, companies, enrolment, files, groups, members, oidc, page, passkeys, record_removal,
    record_types, records, shares, sign_in,
};
use crate::key_routes;
use crate::state::AppState;
use tessari_vault_constants::FILE_PART_MAX_BYTES;

/// One route: its method, its path pattern, and its handler.
pub struct Route {
    /// The method.
    pub method: Method,
    /// The path pattern, with `{name}` segments.
    pub path: &'static str,
    handler: MethodRouter<AppState>,
}

pub(crate) fn route(method: Method, path: &'static str, handler: MethodRouter<AppState>) -> Route {
    Route {
        method,
        path,
        handler,
    }
}

/// Every route the API serves.
#[must_use]
pub fn routes() -> Vec<Route> {
    let mut table = vec![
        route(Method::GET, "/api/me", get(companies::me)),
        route(Method::GET, "/api/companies", get(companies::list)),
        route(Method::POST, "/api/companies", post(companies::create)),
        route(
            Method::POST,
            "/api/companies/{company}/suspend",
            post(companies::suspend),
        ),
        route(
            Method::POST,
            "/api/companies/{company}/resume",
            post(companies::resume),
        ),
        route(
            Method::GET,
            "/api/companies/{company}/members",
            get(members::list),
        ),
        route(
            Method::GET,
            "/api/companies/{company}/audit",
            get(audit::list),
        ),
        route(
            Method::GET,
            "/api/companies/{company}/audit/verify",
            get(audit::verify),
        ),
        route(
            Method::POST,
            "/api/companies/{company}/members",
            post(members::add),
        ),
        route(
            Method::PATCH,
            "/api/companies/{company}/members/{account}",
            patch(members::change),
        ),
        route(
            Method::DELETE,
            "/api/companies/{company}/members/{account}",
            delete(members::remove),
        ),
        route(
            Method::POST,
            "/api/companies/{company}/members/{account}/enrolment",
            post(enrolment::issue),
        ),
        route(Method::GET, "/api/sign-in/methods", get(sign_in::methods)),
        route(Method::POST, "/api/sign-in", post(sign_in::password)),
        route(Method::POST, "/api/sign-in/code", post(sign_in::code)),
        route(Method::DELETE, "/api/session", delete(sign_in::sign_out)),
        route(Method::GET, "/api/sign-in/oidc", get(oidc::start)),
        route(Method::GET, "/api/sign-in/oidc/return", get(oidc::returned)),
        route(Method::POST, "/api/me/identities/oidc", post(oidc::link)),
        route(
            Method::POST,
            "/api/me/passkeys/begin",
            post(passkeys::begin_registration),
        ),
        route(Method::POST, "/api/me/passkeys", post(passkeys::register)),
        route(
            Method::POST,
            "/api/sign-in/passkey/begin",
            post(passkeys::begin_sign_in),
        ),
        route(
            Method::POST,
            "/api/sign-in/passkey",
            post(passkeys::sign_in),
        ),
        route(
            Method::POST,
            "/api/me/identities/oidc/return",
            post(oidc::link_returned),
        ),
        route(Method::POST, "/api/enrolment/begin", post(enrolment::begin)),
        route(
            Method::POST,
            "/api/enrolment/complete",
            post(enrolment::complete),
        ),
        route(
            Method::GET,
            "/api/companies/{company}/groups",
            get(groups::list),
        ),
        route(
            Method::POST,
            "/api/companies/{company}/groups",
            post(groups::create),
        ),
        route(
            Method::PATCH,
            "/api/companies/{company}/groups/{group}",
            patch(groups::rename),
        ),
        route(
            Method::DELETE,
            "/api/companies/{company}/groups/{group}",
            delete(groups::delete),
        ),
        route(
            Method::GET,
            "/api/companies/{company}/groups/{group}/members",
            get(groups::members),
        ),
        route(
            Method::POST,
            "/api/companies/{company}/groups/{group}/members",
            post(groups::add_member),
        ),
        route(
            Method::DELETE,
            "/api/companies/{company}/groups/{group}/members/{account}",
            delete(groups::remove_member),
        ),
        route(
            Method::GET,
            "/api/companies/{company}/records",
            get(records::list),
        ),
        route(
            Method::POST,
            "/api/companies/{company}/records",
            post(records::create),
        ),
        route(
            Method::GET,
            "/api/companies/{company}/records/{record}",
            get(records::read),
        ),
        route(
            Method::PUT,
            "/api/companies/{company}/records/{record}",
            put(records::revise),
        ),
        route(
            Method::DELETE,
            "/api/companies/{company}/records/{record}",
            delete(record_removal::delete),
        ),
        route(
            Method::POST,
            "/api/companies/{company}/records/{record}/shares",
            post(shares::create),
        ),
        route(
            Method::DELETE,
            "/api/companies/{company}/records/{record}/shares/{target}",
            delete(shares::remove),
        ),
        route(
            Method::GET,
            "/api/companies/{company}/record-types",
            get(record_types::list),
        ),
        route(
            Method::POST,
            "/api/companies/{company}/record-types",
            post(record_types::define),
        ),
        route(
            Method::POST,
            "/api/companies/{company}/records/{record}/files",
            post(files::create),
        ),
        route(
            Method::PUT,
            "/api/companies/{company}/records/{record}/files/{file}",
            put(files::write).layer(DefaultBodyLimit::max(part_limit())),
        ),
        route(
            Method::GET,
            "/api/companies/{company}/records/{record}/files/{file}",
            get(files::read),
        ),
    ];
    table.extend(key_routes::routes());
    table.extend(
        page::ASSETS
            .iter()
            .map(|(path, _, _)| route(Method::GET, path, get(page::asset))),
    );
    table
}

/// The largest file part a request body may carry.
fn part_limit() -> usize {
    usize::try_from(FILE_PART_MAX_BYTES).unwrap_or(usize::MAX)
}

/// The API's router, built from [`routes`].
pub fn router(state: AppState) -> Router {
    let mut by_path: BTreeMap<&'static str, MethodRouter<AppState>> = BTreeMap::new();
    for Route { path, handler, .. } in routes() {
        let merged = match by_path.remove(path) {
            Some(existing) => existing.merge(handler),
            None => handler,
        };
        by_path.insert(path, merged);
    }
    by_path
        .into_iter()
        .fold(Router::new(), |router, (path, handler)| {
            router.route(path, handler)
        })
        .with_state(state)
}
