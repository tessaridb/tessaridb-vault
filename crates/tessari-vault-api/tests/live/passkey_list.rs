//! A person lists their own passkeys and removes one; a removed passkey signs nobody in, nobody else can remove it,
//! and adding and removing one go on the trail of each company the person belongs to.

use axum::http::{Method, StatusCode};
use serde_json::Value;

use crate::authenticator::{UP, UV};
use crate::passkeys::{begin_sign_in, failed, registered, sign_in};
use crate::world::{Cast, PASSKEY_ORIGIN, World};

async fn listed(world: &World, token: &str) -> Vec<String> {
    let (status, body) = world
        .call(Method::GET, "/api/me/passkeys", Some(token), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["passkeys"]
        .as_array()
        .expect("a list")
        .iter()
        .map(|key| {
            assert!(key["created"].is_i64(), "{key}");
            assert!(key["backup_eligible"].is_boolean(), "{key}");
            key["credential"].as_str().expect("an id").to_owned()
        })
        .collect()
}

async fn trail(world: &World, cast: &Cast) -> Vec<(String, String)> {
    let (status, body) = world
        .call(
            Method::GET,
            &format!("/api/companies/{}/audit", cast.company_a),
            Some(&cast.owner_a),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["events"]
        .as_array()
        .expect("events")
        .iter()
        .filter(|event| {
            event["action"]
                .as_str()
                .is_some_and(|a| a.starts_with("passkey."))
        })
        .map(|event: &Value| {
            (
                event["action"].as_str().unwrap_or_default().to_owned(),
                event["target"].as_str().unwrap_or_default().to_owned(),
            )
        })
        .collect()
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn a_person_removes_their_own_passkey_and_nobody_elses() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let first = registered(&world, &cast, b"credential-first").await;
    let second = registered(&world, &cast, b"credential-second").await;
    let mut both = listed(&world, &cast.member_a).await;
    both.sort();
    let mut expected = vec![first.id(), second.id()];
    expected.sort();
    assert_eq!(both, expected);
    assert!(
        listed(&world, &cast.owner_a).await.is_empty(),
        "another person's passkeys listed"
    );

    let path = format!("/api/me/passkeys/{}", first.id());
    let (status, body) = world
        .call(Method::DELETE, &path, Some(&cast.owner_a), None)
        .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "another person removed it: {body}"
    );
    assert_eq!(body["error_code"], "not_found");
    assert_eq!(listed(&world, &cast.member_a).await.len(), 2);

    let (status, body) = world
        .call(Method::DELETE, &path, Some(&cast.member_a), None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    assert_eq!(listed(&world, &cast.member_a).await, [second.id()]);
    let (status, again) = world
        .call(Method::DELETE, &path, Some(&cast.member_a), None)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "removed twice: {again}");

    let challenge = begin_sign_in(&world).await;
    let removed = first.assert(&challenge, PASSKEY_ORIGIN, UP | UV, 1, &cast.member_a_id);
    assert!(
        failed(&sign_in(&world, removed).await),
        "a removed passkey signed in"
    );
    let challenge = begin_sign_in(&world).await;
    let kept = second.assert(&challenge, PASSKEY_ORIGIN, UP | UV, 1, &cast.member_a_id);
    let (status, body) = sign_in(&world, kept).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    // Newest first, as the trail reads.
    let member = cast.member_a_id.clone();
    assert_eq!(
        trail(&world, &cast).await,
        [
            ("passkey.remove".to_owned(), member.clone()),
            ("passkey.add".to_owned(), member.clone()),
            ("passkey.add".to_owned(), member),
        ]
    );
}
