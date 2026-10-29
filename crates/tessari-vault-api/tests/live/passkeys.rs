//! Passkeys: a member registers one and signs in with it; every response that is not this ceremony's, this relying
//! party's, this credential's or this account's completes nothing.

use axum::http::{Method, StatusCode};
use serde_json::Value;

use crate::authenticator::{Authenticator, UP, UV};
use crate::world::{Cast, PASSKEY_ORIGIN, PASSKEY_RP_ID, World};

async fn begin_registration(world: &World, token: &str) -> String {
    let (status, body) = world
        .call(Method::POST, "/api/me/passkeys/begin", Some(token), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["rp_id"], PASSKEY_RP_ID, "{body}");
    body["challenge"].as_str().expect("a challenge").to_owned()
}

async fn register(world: &World, token: &str, response: Value) -> (StatusCode, Value) {
    world
        .call(
            Method::POST,
            "/api/me/passkeys",
            Some(token),
            Some(response),
        )
        .await
}

pub(crate) async fn begin_sign_in(world: &World) -> String {
    let (status, body) = world
        .call(Method::POST, "/api/sign-in/passkey/begin", None, None)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["challenge"].as_str().expect("a challenge").to_owned()
}

pub(crate) async fn sign_in(world: &World, assertion: Value) -> (StatusCode, Value) {
    world
        .call(Method::POST, "/api/sign-in/passkey", None, Some(assertion))
        .await
}

pub(crate) fn failed(answer: &(StatusCode, Value)) -> bool {
    answer.0 == StatusCode::UNAUTHORIZED && answer.1["error_code"] == "sign_in_failed"
}

fn refused(answer: &(StatusCode, Value)) -> bool {
    answer.0 == StatusCode::BAD_REQUEST && answer.1["error_code"] == "passkey_refused"
}

/// A passkey registered for the member.
pub(crate) async fn registered(world: &World, cast: &Cast, id: &[u8]) -> Authenticator {
    let key = Authenticator::new(id, PASSKEY_RP_ID);
    let challenge = begin_registration(world, &cast.member_a).await;
    let (status, body) = register(
        world,
        &cast.member_a,
        key.create(&challenge, PASSKEY_ORIGIN),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["credential_id"], key.id(), "{body}");
    key
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn a_registered_passkey_signs_its_account_in_once_per_ceremony() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let key = registered(&world, &cast, b"credential-one").await;
    let challenge = begin_sign_in(&world).await;
    // Counter 0, as a synced passkey reports it: only the spent ceremony stops this assertion being used again.
    let assertion = key.assert(&challenge, PASSKEY_ORIGIN, UP | UV, 0, &cast.member_a_id);
    let (status, body) = sign_in(&world, assertion.clone()).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let session = body["session"].as_str().expect("a session");
    let (status, me) = world
        .call(Method::GET, "/api/me", Some(session), None)
        .await;
    assert_eq!(
        (status, me["account"]["id"].as_str()),
        (StatusCode::OK, Some(cast.member_a_id.as_str())),
        "{me}"
    );
    assert!(
        failed(&sign_in(&world, assertion).await),
        "a ceremony is answered once"
    );
    let (status, body) = world
        .call(
            Method::POST,
            "/api/me/passkeys/begin",
            Some(&cast.member_a),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["exclude"],
        serde_json::json!([key.id()]),
        "a new registration excludes it"
    );
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn a_registration_that_is_not_this_ceremony_or_this_party_is_refused() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let key = Authenticator::new(b"credential-two", PASSKEY_RP_ID);
    let challenge = begin_registration(&world, &cast.member_a).await;
    let response = key.create(&challenge, "https://elsewhere.test");
    assert!(
        refused(&register(&world, &cast.member_a, response).await),
        "another origin"
    );
    let other_party = Authenticator::new(b"credential-two", "elsewhere.test");
    let challenge = begin_registration(&world, &cast.member_a).await;
    let response = other_party.create(&challenge, PASSKEY_ORIGIN);
    assert!(
        refused(&register(&world, &cast.member_a, response).await),
        "another relying party"
    );
    let challenge = begin_registration(&world, &cast.admin_a).await;
    let response = key.create(&challenge, PASSKEY_ORIGIN);
    assert!(
        refused(&register(&world, &cast.member_a, response).await),
        "another account's ceremony"
    );
    let challenge = begin_sign_in(&world).await;
    let response = key.create(&challenge, PASSKEY_ORIGIN);
    assert!(
        refused(&register(&world, &cast.member_a, response).await),
        "a sign-in ceremony"
    );
    let challenge = begin_registration(&world, &cast.member_a).await;
    let (status, body) = register(
        &world,
        &cast.member_a,
        key.create(&challenge, PASSKEY_ORIGIN),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let challenge = begin_registration(&world, &cast.admin_a).await;
    let (status, body) = register(
        &world,
        &cast.admin_a,
        key.create(&challenge, PASSKEY_ORIGIN),
    )
    .await;
    assert_eq!(
        (status, body["error_code"].as_str()),
        (StatusCode::CONFLICT, Some("passkey_registered")),
        "{body}"
    );
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn an_assertion_that_is_not_this_credential_or_account_signs_nobody_in() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let key = registered(&world, &cast, b"credential-three").await;
    let member = cast.member_a_id.as_str();
    let impostor = Authenticator::new(b"credential-three", PASSKEY_RP_ID);
    let unknown = Authenticator::new(b"credential-unknown", PASSKEY_RP_ID);
    let rows: [(&str, &Authenticator, &str, u8, &str); 5] = [
        (
            "another origin",
            &key,
            "https://elsewhere.test",
            UP | UV,
            member,
        ),
        (
            "a signature by another key",
            &impostor,
            PASSKEY_ORIGIN,
            UP | UV,
            member,
        ),
        (
            "an unknown credential",
            &unknown,
            PASSKEY_ORIGIN,
            UP | UV,
            member,
        ),
        ("the user not verified", &key, PASSKEY_ORIGIN, UP, member),
        (
            "another account's user handle",
            &key,
            PASSKEY_ORIGIN,
            UP | UV,
            cast.admin_a_id.as_str(),
        ),
    ];
    for (row, by, origin, flags, user) in rows {
        let challenge = begin_sign_in(&world).await;
        assert!(
            failed(&sign_in(&world, by.assert(&challenge, origin, flags, 1, user)).await),
            "{row}"
        );
    }
    let challenge = begin_registration(&world, &cast.member_a).await;
    let assertion = key.assert(&challenge, PASSKEY_ORIGIN, UP | UV, 1, member);
    assert!(
        failed(&sign_in(&world, assertion).await),
        "a registration ceremony"
    );
    let challenge = begin_sign_in(&world).await;
    let (status, body) = sign_in(
        &world,
        key.assert(&challenge, PASSKEY_ORIGIN, UP | UV, 5, member),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "the counter moves to 5: {body}"
    );
    let challenge = begin_sign_in(&world).await;
    assert!(
        failed(
            &sign_in(
                &world,
                key.assert(&challenge, PASSKEY_ORIGIN, UP | UV, 5, member)
            )
            .await
        ),
        "a counter that did not move forward: a possible clone"
    );
}
