//! Sign-in through an OpenID Connect issuer, against a stub issuer: linking an identity, signing in with it, and every
//! return that must complete nothing.

use axum::http::header::{LOCATION, SET_COOKIE};
use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use crate::issuer::{Grant, Issuer};
use crate::world::{Cast, World};

/// The query parameter `name` of the address `address`.
fn param(address: &str, name: &str) -> String {
    let query = address.split_once('?').map_or("", |(_, query)| query);
    url::form_urlencoded::parse(query.as_bytes())
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.into_owned())
        .unwrap_or_else(|| panic!("no {name} in {address}"))
}

/// A request begun at the issuer: its `state`, and the grant its code will carry unless a row changes it.
struct Begun {
    state: String,
    grant: Grant,
}

fn begun(address: &str, subject: &str) -> Begun {
    Begun {
        state: param(address, "state"),
        grant: Grant {
            challenge: param(address, "code_challenge"),
            subject: subject.to_owned(),
            nonce: param(address, "nonce"),
            issuer: None,
            audience: None,
        },
    }
}

/// The issuer's return for `begun`, with `code` granted as `grant`.
fn returned(issuer: &Issuer, begun: &Begun, code: &str, grant: Grant) -> String {
    issuer.grant(code, grant);
    format!(
        "code={code}&state={}&iss={}",
        begun.state,
        url::form_urlencoded::byte_serialize(issuer.issuer().as_bytes()).collect::<String>()
    )
}

/// Begin a sign-in: the request and the binding cookie the browser got.
async fn start(world: &World, subject: &str) -> (Begun, String) {
    let (status, headers, _) = world
        .call_with(Method::GET, "/api/sign-in/oidc", &[], None)
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    let location = headers[LOCATION].to_str().expect("an address").to_owned();
    let cookie = headers[SET_COOKIE].to_str().expect("a cookie").to_owned();
    assert!(
        cookie.contains("HttpOnly") && cookie.contains("Secure") && cookie.contains("SameSite=Lax"),
        "{cookie}"
    );
    let pair = cookie.split(';').next().expect("a pair").to_owned();
    (begun(&location, subject), pair)
}

async fn sign_in_return(world: &World, query: &str, cookie: Option<&str>) -> (StatusCode, Value) {
    let headers: Vec<(&str, String)> = cookie
        .map(|pair| ("cookie", pair.to_owned()))
        .into_iter()
        .collect();
    let (status, _, body) = world
        .call_with(
            Method::GET,
            &format!("/api/sign-in/oidc/return?{query}"),
            &headers,
            None,
        )
        .await;
    (status, body)
}

async fn begin_link(world: &World, token: &str, subject: &str) -> Begun {
    let (status, body) = world
        .call(Method::POST, "/api/me/identities/oidc", Some(token), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    begun(body["address"].as_str().expect("an address"), subject)
}

async fn link_return(world: &World, token: &str, query: &str) -> (StatusCode, Value) {
    world
        .call(
            Method::POST,
            "/api/me/identities/oidc/return",
            Some(token),
            Some(json!({ "query": query })),
        )
        .await
}

/// A change one row makes to the grant its code carries.
type Change = fn(&mut Grant);

fn failed(answer: &(StatusCode, Value)) -> bool {
    answer.0 == StatusCode::UNAUTHORIZED && answer.1["error_code"] == "sign_in_failed"
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn a_linked_identity_signs_in_and_an_unlinked_one_does_not() {
    let issuer = Issuer::start().await;
    let world = World::with_issuer(issuer.issuer()).await;
    let cast = Cast::new(&world).await;
    let (begun_a, cookie) = start(&world, "alice").await;
    let query = returned(&issuer, &begun_a, "c1", begun_a.grant.clone());
    assert!(
        failed(&sign_in_return(&world, &query, Some(&cookie)).await),
        "not linked yet"
    );
    let link = begin_link(&world, &cast.member_a, "alice").await;
    let query = returned(&issuer, &link, "c2", link.grant.clone());
    let (status, body) = link_return(&world, &cast.member_a, &query).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let (begun_b, cookie) = start(&world, "alice").await;
    let query = returned(&issuer, &begun_b, "c3", begun_b.grant.clone());
    let (status, body) = sign_in_return(&world, &query, Some(&cookie)).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let session = body["session"].as_str().expect("a session");
    let (status, me) = world
        .call(Method::GET, "/api/me", Some(session), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{me}");
    assert_eq!(me["account"]["id"], cast.member_a_id, "{me}");
    assert!(
        failed(&sign_in_return(&world, &query, Some(&cookie)).await),
        "a return completes once"
    );
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn a_return_whose_token_or_request_is_not_this_one_completes_nothing() {
    let issuer = Issuer::start().await;
    let world = World::with_issuer(issuer.issuer()).await;
    let cast = Cast::new(&world).await;
    let link = begin_link(&world, &cast.member_a, "bob").await;
    let query = returned(&issuer, &link, "l1", link.grant.clone());
    assert_eq!(
        link_return(&world, &cast.member_a, &query).await.0,
        StatusCode::NO_CONTENT
    );
    let rows: [(&str, Change); 4] = [
        ("another issuer in the ID token", |grant| {
            grant.issuer = Some("http://127.0.0.1:1".to_owned())
        }),
        ("another audience", |grant| {
            grant.audience = Some("someone-else".to_owned())
        }),
        ("another nonce", |grant| {
            grant.nonce = "not-the-nonce".to_owned()
        }),
        ("another PKCE challenge", |grant| {
            grant.challenge = "not-the-challenge".to_owned()
        }),
    ];
    for (turn, (row, change)) in rows.into_iter().enumerate() {
        let (begun, cookie) = start(&world, "bob").await;
        let mut grant = begun.grant.clone();
        change(&mut grant);
        let query = returned(&issuer, &begun, &format!("r{turn}"), grant);
        assert!(
            failed(&sign_in_return(&world, &query, Some(&cookie)).await),
            "{row}"
        );
    }
    let (begun, cookie) = start(&world, "bob").await;
    let query = returned(&issuer, &begun, "s1", begun.grant.clone());
    let forged = query.replace(&begun.state, "a-state-nobody-issued");
    assert!(
        failed(&sign_in_return(&world, &forged, Some(&cookie)).await),
        "another state"
    );
    let (status, body) = sign_in_return(&world, &query, Some(&cookie)).await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "a state nobody issued spends nothing, so the real return still completes: {body}"
    );
    let (begun, cookie) = start(&world, "bob").await;
    let query = returned(&issuer, &begun, "s2", begun.grant.clone());
    let wrong_iss = query.replace("&iss=", "&iss=http%3A%2F%2F127.0.0.1%3A1&was=");
    assert!(
        failed(&sign_in_return(&world, &wrong_iss, Some(&cookie)).await),
        "another iss in the return"
    );
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn a_return_completes_only_for_the_party_that_began_it() {
    let issuer = Issuer::start().await;
    let world = World::with_issuer(issuer.issuer()).await;
    let cast = Cast::new(&world).await;
    let link = begin_link(&world, &cast.member_a, "carol").await;
    let query = returned(&issuer, &link, "k1", link.grant.clone());
    assert_eq!(
        link_return(&world, &cast.member_a, &query).await.0,
        StatusCode::NO_CONTENT
    );
    // Login CSRF: the attacker's return handed to a browser that did not begin it.
    let (begun, _) = start(&world, "carol").await;
    let query = returned(&issuer, &begun, "k2", begun.grant.clone());
    assert!(
        failed(&sign_in_return(&world, &query, None).await),
        "no binding cookie"
    );
    let (begun, cookie) = start(&world, "carol").await;
    let (_, other) = start(&world, "carol").await;
    let query = returned(&issuer, &begun, "k3", begun.grant.clone());
    assert!(
        failed(&sign_in_return(&world, &query, Some(&other)).await),
        "another browser's cookie"
    );
    assert!(
        failed(&sign_in_return(&world, &query, Some(&cookie)).await),
        "a refused return spent its request"
    );
    // Link CSRF: the attacker's link request completed by someone else's session, or at the sign-in return.
    let link = begin_link(&world, &cast.admin_a, "dave").await;
    let query = returned(&issuer, &link, "k4", link.grant.clone());
    assert!(
        failed(&link_return(&world, &cast.member_a, &query).await),
        "another account's link"
    );
    let link = begin_link(&world, &cast.admin_a, "dave").await;
    let query = returned(&issuer, &link, "k5", link.grant.clone());
    assert!(
        failed(&sign_in_return(&world, &query, Some(&cookie)).await),
        "a link at the sign-in return"
    );
    let (begun, _) = start(&world, "dave").await;
    let query = returned(&issuer, &begun, "k6", begun.grant.clone());
    assert!(
        failed(&link_return(&world, &cast.admin_a, &query).await),
        "a sign-in at the link return"
    );
    // An identity linked to one account is never linked to a second.
    let link = begin_link(&world, &cast.admin_a, "carol").await;
    let query = returned(&issuer, &link, "k7", link.grant.clone());
    let (status, body) = link_return(&world, &cast.admin_a, &query).await;
    assert_eq!(
        (status, body["error_code"].as_str()),
        (StatusCode::CONFLICT, Some("identity_linked")),
        "{body}"
    );
}
