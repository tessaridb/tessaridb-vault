//! Enrolment: a link an admin issues, a member's own password and TOTP, once.

use std::time::{SystemTime, UNIX_EPOCH};

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};
use tessari_vault_core::totp;

use crate::world::{Cast, World, add_member};

/// `text` decoded from unpadded RFC 4648 base32.
fn base32(text: &str) -> Vec<u8> {
    let (mut bytes, mut buffer, mut held) = (Vec::new(), 0_u32, 0_u32);
    for character in text.bytes() {
        let value = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567"
            .iter()
            .position(|known| *known == character)
            .and_then(|index| u32::try_from(index).ok())
            .expect("a base32 character");
        buffer = (buffer << 5) | value;
        held = held.saturating_add(5);
        if held >= 8 {
            held = held.saturating_sub(8);
            bytes.push(u8::try_from((buffer >> held) & 0xff).expect("one byte"));
        }
    }
    bytes
}

/// The authenticator's code for `secret` now, and a code that is not it.
fn codes(secret: &str) -> (String, String) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("after 1970")
        .as_secs();
    let right = totp::code_at(&base32(secret), now / 30);
    (
        format!("{right:06}"),
        format!("{:06}", right.wrapping_add(500_000) % 1_000_000),
    )
}

async fn issue(world: &World, by: &str, company: &str, account: &str) -> (StatusCode, Value) {
    world
        .call(
            Method::POST,
            &format!("/api/companies/{company}/members/{account}/enrolment"),
            Some(by),
            None,
        )
        .await
}

async fn begin(world: &World, token: &str) -> (StatusCode, Value) {
    world
        .call(
            Method::POST,
            "/api/enrolment/begin",
            None,
            Some(json!({ "token": token })),
        )
        .await
}

async fn complete(world: &World, token: &str, password: &str, code: &str) -> (StatusCode, Value) {
    world
        .call(
            Method::POST,
            "/api/enrolment/complete",
            None,
            Some(json!({ "token": token, "password": password, "code": code })),
        )
        .await
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn a_member_enrols_once_with_their_own_password_and_code_and_is_signed_in() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let a = &cast.company_a;
    let new = add_member(&world, a, &cast.admin_a, "new@alpha.test", "member").await;
    let (status, body) = issue(&world, &cast.admin_a, a, &new).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let token = body["token"].as_str().expect("a token").to_owned();
    let (status, body) = begin(&world, &token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let secret = body["secret"].as_str().expect("a secret").to_owned();
    assert!(
        body["uri"].as_str().is_some_and(
            |uri| uri.starts_with("otpauth://totp/TessariDB%20Vault:new%40alpha.test?secret=")
        ),
        "{body}"
    );
    let (right, wrong) = codes(&secret);
    let password = "a passphrase of my own";
    let (status, body) = complete(&world, &token, "new@alpha.test and more", &right).await;
    assert_eq!(
        (status, body["error_code"].as_str()),
        (StatusCode::BAD_REQUEST, Some("weak_password"))
    );
    let (status, body) = complete(&world, &token, password, &wrong).await;
    assert_eq!(
        (status, body["error_code"].as_str()),
        (StatusCode::BAD_REQUEST, Some("wrong_code"))
    );
    let (status, body) = complete(&world, &token, password, &right).await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "the link survived both refusals: {body}"
    );
    let session = body["session"].as_str().expect("a session").to_owned();
    let (status, body) = world
        .call(Method::GET, "/api/me", Some(&session), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["account"]["email"], "new@alpha.test", "{body}");
    assert_eq!(
        complete(&world, &token, password, &right).await.0,
        StatusCode::NOT_FOUND,
        "used once"
    );
    assert_eq!(
        begin(&world, &token).await.0,
        StatusCode::NOT_FOUND,
        "used once"
    );
    let (status, body) = issue(&world, &cast.admin_a, a, &new).await;
    assert_eq!(
        (status, body["error_code"].as_str()),
        (StatusCode::CONFLICT, Some("enrolled"))
    );
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn a_newer_link_replaces_the_older_and_any_link_that_is_not_live_is_one_answer() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let a = &cast.company_a;
    let first = issue(&world, &cast.admin_a, a, &cast.member_a_id).await.1["token"]
        .as_str()
        .expect("a token")
        .to_owned();
    let second = issue(&world, &cast.admin_a, a, &cast.member_a_id).await.1["token"]
        .as_str()
        .expect("a token")
        .to_owned();
    let (status, replaced) = begin(&world, &first).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "the older link: {replaced}");
    assert_eq!(
        begin(&world, &second).await.0,
        StatusCode::OK,
        "the newer link"
    );
    for token in ["zz", "", &"ab".repeat(32), &"AB".repeat(32)] {
        assert_eq!(
            begin(&world, token).await,
            (StatusCode::NOT_FOUND, replaced.clone()),
            "{token:?}"
        );
        assert_eq!(
            complete(&world, token, "a passphrase of my own", "123456").await,
            (StatusCode::NOT_FOUND, replaced.clone()),
            "{token:?}"
        );
    }
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn only_someone_who_manages_the_member_issues_their_link() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let a = &cast.company_a;
    let (status, body) = issue(&world, &cast.admin_a, a, &cast.owner_a_id).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "an admin does not manage an owner: {body}"
    );
    let (status, body) = issue(&world, &cast.owner_a, a, &cast.admin_a_id).await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "an owner manages an admin: {body}"
    );
    let (status, body) = issue(&world, &cast.admin_b, a, &cast.member_a_id).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "another company's admin sees no such member: {body}"
    );
    let (status, body) = issue(&world, &cast.admin_a, &cast.company_b, &cast.admin_b_id).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "nor the other way round: {body}"
    );
}
