//! The route table. The router is built from it and nothing else, so it is also the source set of the
//! enforcement-point coverage matrix: a route added anywhere else would not be served.

use std::collections::BTreeMap;

use axum::Router;
use axum::http::Method;
use axum::routing::{MethodRouter, delete, get, patch, post};

use crate::handlers::{companies, enrolment, groups, members, oidc, passkeys, sign_in};
use crate::state::AppState;

/// One route: its method, its path pattern, and its handler.
pub struct Route {
    /// The method.
    pub method: Method,
    /// The path pattern, with `{name}` segments.
    pub path: &'static str,
    handler: MethodRouter<AppState>,
}

fn route(method: Method, path: &'static str, handler: MethodRouter<AppState>) -> Route {
    Route {
        method,
        path,
        handler,
    }
}

/// Every route the API serves.
#[must_use]
pub fn routes() -> Vec<Route> {
    vec![
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
    ]
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
