//! The vault against a running TessariDB OAuth server, over its published endpoints only: a member links their
//! identity there, then signs in with it, and the same return completes once.
//!
//! Needs `TESSARIDB_BIN`, `TESSARIDB_OAUTH_E2E_ISSUER` (`https://<host>/<company>`), `TESSARIDB_OAUTH_E2E_ADMIN`
//! (`<console>/admin/<company>`), `TESSARIDB_OAUTH_E2E_OPERATOR` and `TESSARIDB_OAUTH_E2E_PASSWORD` (an operator of
//! that company with the `users` and `clients` roles and no authenticator app yet, which the run adds), and
//! `SSL_CERT_FILE` for a server whose certificate a private authority signed. Each run registers a client and a user
//! of its own and needs a fresh operator.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::http::{Method, StatusCode};
use reqwest::header::{LOCATION, ORIGIN};
use ring::rand::{SecureRandom, SystemRandom};
use serde_json::{Value, json};
use tessari_vault_core::totp;

use crate::enrolment::base32;
use crate::issuer::b64;
use crate::world::{Cast, World};

const REDIRECT: &str = "https://vault.e2e.example/sign-in/return";
const RESOURCE: &str = "https://vault.e2e.example/";

fn variable(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("{name} is not set"))
}

fn random(bytes: usize) -> String {
    let mut buffer = vec![0_u8; bytes];
    SystemRandom::new().fill(&mut buffer).expect("random bytes");
    b64(&buffer)
}

fn step() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("after 1970")
        .as_secs()
        / 30
}

fn query_of(url: &str, name: &str) -> String {
    let query = url.split_once('?').map_or("", |(_, query)| query);
    url::form_urlencoded::parse(query.as_bytes())
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.into_owned())
        .unwrap_or_else(|| panic!("no {name} in {url}"))
}

/// A browser as far as the issuer's pages need one: cookies kept, redirects read rather than followed.
struct Browser {
    http: reqwest::Client,
}

impl Browser {
    fn new() -> Self {
        let http = reqwest::Client::builder()
            .cookie_store(true)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("a client");
        Self { http }
    }

    async fn location(&self, url: &str) -> String {
        let answer = self.http.get(url).send().await.expect("an answer");
        assert!(
            answer.status().is_redirection(),
            "{url}: {}",
            answer.status()
        );
        let target = answer.headers()[LOCATION]
            .to_str()
            .expect("a Location")
            .to_owned();
        url::Url::parse(url)
            .and_then(|base| base.join(&target))
            .expect("a target")
            .into()
    }

    async fn post(&self, url: &str, body: &Value, headers: &[(&str, &str)]) -> (StatusCode, Value) {
        let mut request = self.http.post(url).json(body);
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        let answer = request.send().await.expect("an answer");
        let status = StatusCode::from_u16(answer.status().as_u16()).expect("a status");
        let text = answer.text().await.expect("a body");
        (status, serde_json::from_str(&text).unwrap_or(Value::Null))
    }

    /// Sign in as `email` from the sign-in `begin` names: where the completed sign-in continues.
    async fn sign_in(
        &self,
        issuer: &str,
        begin: Value,
        email: &str,
        password: &str,
        app: Option<&[u8]>,
    ) -> String {
        let api = format!("{issuer}/login/api");
        let first = if app.is_some() {
            "second_factor"
        } else {
            "complete"
        };
        let mut steps = vec![
            ("begin", begin, "identify"),
            ("identify", json!({"email": email}), "first_factor"),
            ("password", json!({"password": password}), first),
        ];
        if let Some(secret) = app {
            steps.push((
                "totp",
                json!({"code": format!("{:06}", totp::code_at(secret, step()))}),
                "complete",
            ));
        }
        let mut answer = Value::Null;
        for (name, body, next) in steps {
            let (status, read) = self.post(&format!("{api}/{name}"), &body, &[]).await;
            assert_eq!(
                (status, read["next"].as_str()),
                (StatusCode::OK, Some(next)),
                "{name}: {read}"
            );
            answer = read;
        }
        answer["continue"]
            .as_str()
            .expect("where to continue")
            .to_owned()
    }

    /// Sign in for the authorization request at `authorize` and allow what it asks: where the browser returns.
    async fn authorize(
        &self,
        issuer: &str,
        authorize: &str,
        user: (&str, &str),
        app: Option<&[u8]>,
    ) -> String {
        let login = self.location(authorize).await;
        assert!(login.starts_with(&format!("{issuer}/login?")), "{login}");
        let begin = json!({"interaction": query_of(&login, "interaction")});
        let resumed = self.sign_in(issuer, begin, user.0, user.1, app).await;
        let after = self.location(&resumed).await;
        self.consented(issuer, after).await
    }

    /// The same, for a user the issuer may still hold a session for: signed in again only when it asks.
    async fn authorize_or_return(
        &self,
        issuer: &str,
        authorize: &str,
        user: (&str, &str),
    ) -> String {
        let first = self.location(authorize).await;
        if first.starts_with(&format!("{issuer}/login?")) {
            return self.authorize(issuer, authorize, user, None).await;
        }
        self.consented(issuer, first).await
    }

    /// Allow what the consent page at `after` asks, when `after` is one: where the browser returns.
    async fn consented(&self, issuer: &str, after: String) -> String {
        if !after.starts_with(&format!("{issuer}/consent?")) {
            return after;
        }
        let interaction = query_of(&after, "interaction");
        let (status, onward) = self
            .post(
                &format!("{issuer}/login/api/consent"),
                &json!({"interaction": interaction, "decision": "allow"}),
                &[],
            )
            .await;
        assert_eq!(status, StatusCode::OK, "consent: {onward}");
        onward["redirect"].as_str().expect("a redirect").to_owned()
    }

    /// A change at the console `admin`, as the console makes it.
    async fn change(&self, admin: &str, path: &str, body: &Value) -> (StatusCode, Value) {
        let origin = url::Url::parse(admin)
            .expect("a URL")
            .origin()
            .ascii_serialization();
        let key = random(16);
        self.post(
            &format!("{admin}/api/{path}"),
            body,
            &[
                (ORIGIN.as_str(), origin.as_str()),
                ("idempotency-key", key.as_str()),
            ],
        )
        .await
    }
}

/// The operator adds an authenticator, signs in to the console, registers the vault as a public client and opens a
/// user with a password: the client id and the user.
async fn prepared(browser: &Browser, issuer: &str, admin: &str) -> (String, String, String) {
    let (operator, password) = (
        variable("TESSARIDB_OAUTH_E2E_OPERATOR"),
        variable("TESSARIDB_OAUTH_E2E_PASSWORD"),
    );
    browser
        .sign_in(issuer, json!({"account": true}), &operator, &password, None)
        .await;
    let (status, begun) = browser
        .post(&format!("{issuer}/account/api/totp/begin"), &json!({}), &[])
        .await;
    assert_eq!(status, StatusCode::OK, "{begun}");
    let app = base32(begun["secret"].as_str().expect("a secret"));
    let enrolled_at = step();
    let code = json!({"code": format!("{:06}", totp::code_at(&app, enrolled_at))});
    let (status, body) = browser
        .post(&format!("{issuer}/account/api/totp/confirm"), &code, &[])
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    // A code is accepted once per 30-second step, so the console sign-in waits for the next one.
    while step() == enrolled_at {
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    let signin = browser.location(&format!("{admin}/signin")).await;
    let callback = browser
        .authorize(issuer, &signin, (&operator, &password), Some(&app))
        .await;
    browser.location(&callback).await;
    let client_id = format!(
        "vault-{}",
        random(6).to_ascii_lowercase().replace(['-', '_'], "x")
    );
    let (status, body) = browser
        .change(
            admin,
            "clients",
            &json!({
                "client_id": client_id, "redirect_uris": [REDIRECT], "scope_ceiling": ["openid"],
                // Every grant there is addressed to one resource; the vault names itself.
                "resources": [RESOURCE],
                "auth_method": "none", "grant_types": ["authorization_code"], "reason": "vault end-to-end run",
            }),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let email = format!("{client_id}@acme.e2e.example");
    let (status, opened) = browser
        .change(
            admin,
            "users",
            &json!({"email": email, "reason": "vault run"}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{opened}");
    let user = opened["user"].as_str().expect("a user");
    let (status, issued) = browser
        .change(
            admin,
            &format!("users/{user}/enrolment"),
            &json!({"reason": "vault run"}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{issued}");
    let user_password = random(18);
    let enrol = json!({"code": issued["code"], "password": user_password});
    let (status, _) = browser
        .post(&format!("{issuer}/login/api/enroll"), &enrol, &[])
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "enrolment");
    (client_id, email, user_password)
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN and a running TessariDB OAuth server: see the module documentation"]
async fn a_member_links_an_identity_at_the_issuer_and_signs_in_with_it() {
    let (issuer, admin) = (
        variable("TESSARIDB_OAUTH_E2E_ISSUER"),
        variable("TESSARIDB_OAUTH_E2E_ADMIN"),
    );
    let operator_browser = Browser::new();
    let (client_id, email, password) = prepared(&operator_browser, &issuer, &admin).await;
    let world = World::registered_at(&issuer, &client_id, REDIRECT).await;
    let cast = Cast::new(&world).await;
    let user = Browser::new();
    let (status, headers, body) = world
        .call_with(
            Method::POST,
            "/api/me/identities/oidc",
            &[("authorization", format!("Bearer {}", cast.member_a))],
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let link_cookie = headers[axum::http::header::SET_COOKIE]
        .to_str()
        .expect("a cookie")
        .split(';')
        .next()
        .expect("a pair")
        .to_owned();
    let back = user
        .authorize(
            &issuer,
            body["address"].as_str().expect("an address"),
            (&email, &password),
            None,
        )
        .await;
    assert!(back.starts_with(REDIRECT), "{back}");
    let query = back.split_once('?').expect("a query").1.to_owned();
    let (status, _, body) = world
        .call_with(
            Method::GET,
            &format!("/api/sign-in/oidc/return?{query}"),
            &[("cookie", link_cookie)],
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "linked: {body}");
    let (status, headers, _) = world
        .call_with(Method::GET, "/api/sign-in/oidc", &[], None)
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    let address = headers[axum::http::header::LOCATION]
        .to_str()
        .expect("an address")
        .to_owned();
    let cookie = headers[axum::http::header::SET_COOKIE]
        .to_str()
        .expect("a cookie");
    let cookie = cookie.split(';').next().expect("a pair").to_owned();
    // The issuer remembers the user's session, so this time it may send them straight back.
    let back = user
        .authorize_or_return(&issuer, &address, (&email, &password))
        .await;
    assert!(back.starts_with(REDIRECT), "{back}");
    let path = format!(
        "/api/sign-in/oidc/return?{}",
        back.split_once('?').expect("a query").1
    );
    let headers = [("cookie", cookie)];
    let (status, _, body) = world.call_with(Method::GET, &path, &headers, None).await;
    assert_eq!(status, StatusCode::CREATED, "signed in: {body}");
    let session = body["session"].as_str().expect("a session");
    let (status, me) = world
        .call(Method::GET, "/api/me", Some(session), None)
        .await;
    assert_eq!(
        (status, me["account"]["id"].as_str()),
        (StatusCode::OK, Some(cast.member_a_id.as_str())),
        "{me}"
    );
    let (status, _, body) = world.call_with(Method::GET, &path, &headers, None).await;
    assert_eq!(
        (status, body["error_code"].as_str()),
        (StatusCode::UNAUTHORIZED, Some("sign_in_failed"))
    );
}
