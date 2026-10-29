//! The web application the server carries in its binary. The page and its assets carry no data (everything the page
//! shows it fetches from `/api` with the member's session, and decrypts itself), so they are served without a session,
//! counted per address like every public route, and each answer carries a policy that lets the page run only its own
//! script and style, talk only to this origin, and never be framed.
//!
//! The bytes are the committed output of `panel/` (`npm run build`), embedded so that building the server never needs
//! Node; `npm run verify` fails when the committed output no longer matches its source.

use axum::http::header::{
    CACHE_CONTROL, CONTENT_SECURITY_POLICY, CONTENT_TYPE, REFERRER_POLICY, X_CONTENT_TYPE_OPTIONS,
};
use axum::http::{HeaderValue, Uri};
use axum::response::{IntoResponse, Response};

use crate::ApiError;
use crate::caller::Unsigned;

/// What the page may load and do: its own script, style and images; requests to this origin only; no form submitted
/// anywhere, no `<base>` rewriting, no framing.
const POLICY: &str = "default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self'; connect-src 'self'; \
                      form-action 'none'; frame-ancestors 'none'; base-uri 'none'";

/// One asset: the path it is served at, its type, its bytes.
type Asset = (&'static str, &'static str, &'static str);

/// The page's assets, each served at its own route in the table.
pub(crate) const ASSETS: [Asset; 4] = [
    (
        "/",
        "text/html; charset=utf-8",
        include_str!("../../assets/index.html"),
    ),
    (
        "/app.js",
        "text/javascript; charset=utf-8",
        include_str!("../../assets/app.js"),
    ),
    (
        "/app.css",
        "text/css; charset=utf-8",
        include_str!("../../assets/app.css"),
    ),
    (
        "/favicon.svg",
        "image/svg+xml",
        include_str!("../../assets/favicon.svg"),
    ),
];

pub(crate) async fn asset(Unsigned(_): Unsigned, uri: Uri) -> Result<Response, ApiError> {
    let (_, kind, body) = ASSETS
        .iter()
        .find(|(path, _, _)| *path == uri.path())
        .ok_or(ApiError::NOT_FOUND)?;
    let mut response = (*body).into_response();
    let headers = response.headers_mut();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static(kind));
    headers.insert(CONTENT_SECURITY_POLICY, HeaderValue::from_static(POLICY));
    headers.insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    headers.insert(REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    // The page and its assets change only with the binary, and a stale page must never outlive an upgrade.
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    Ok(response)
}
