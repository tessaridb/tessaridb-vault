//! Deleting a company: an owner asks with a statement their own registered key signed, so the server cannot ask on
//! an owner's behalf, and nobody else may ask.

use std::time::{SystemTime, UNIX_EPOCH};

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};
use tessari_vault_core::company_drop::{Act, statement};

use crate::keys::key_set;
use crate::records::encoded;
use crate::statements::Signer;
use crate::world::{Cast, World, add_member};

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("after 1970")
        .as_secs()
}

/// A second owner of company A who keeps a key set whose signing key `signer` holds.
async fn signing_owner(world: &World, cast: &Cast, signer: &Signer) -> String {
    let account = add_member(
        world,
        &cast.company_a,
        &cast.owner_a,
        "second-owner@alpha.test",
        "owner",
    )
    .await;
    let token = world.token(&account).await;
    let mut keys = key_set(&"66".repeat(16), 6);
    keys["signing_public"] = Value::from(encoded(&signer.public()));
    let (status, body) = world
        .call(
            Method::PUT,
            &format!("/api/companies/{}/keys/me", cast.company_a),
            Some(&token),
            Some(keys),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    token
}

fn asked(company: &str, by: &Signer, at: u64) -> Value {
    json!({
        "at": at,
        "signature": encoded(&by.stored(&statement(Act::Request, company, at))),
    })
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn an_owner_asks_with_a_statement_their_own_key_signed_and_nobody_else_may() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let signer = Signer::new();
    let owner = signing_owner(&world, &cast, &signer).await;
    let drop = format!("/api/companies/{}/drop", cast.company_a);
    let a = cast.company_a.as_str();
    for (who, token, body, expected, code) in [
        (
            "an admin",
            &cast.admin_a,
            asked(a, &signer, now()),
            StatusCode::FORBIDDEN,
            Some("forbidden"),
        ),
        (
            "another key",
            &owner,
            asked(a, &Signer::new(), now()),
            StatusCode::CONFLICT,
            Some("drop_unsigned"),
        ),
        (
            "an hour old",
            &owner,
            asked(a, &signer, now().saturating_sub(3600)),
            StatusCode::CONFLICT,
            Some("drop_unsigned"),
        ),
        (
            "another company's statement",
            &owner,
            asked(&cast.company_b, &signer, now()),
            StatusCode::CONFLICT,
            Some("drop_unsigned"),
        ),
    ] {
        let (status, answer) = world
            .call(Method::POST, &drop, Some(token), Some(body))
            .await;
        assert_eq!(
            (status, answer["error_code"].as_str().map(str::to_owned)),
            (expected, code.map(str::to_owned)),
            "{who}: {answer}"
        );
    }
    let (status, answer) = world
        .call(
            Method::POST,
            &drop,
            Some(&owner),
            Some(asked(a, &signer, now())),
        )
        .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{answer}");
    assert_eq!(answer["state"], "requested", "{answer}");
    let (status, trail) = world
        .call(
            Method::GET,
            &format!("/api/companies/{a}/audit"),
            Some(&cast.owner_a),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{trail}");
    assert!(
        trail.to_string().contains("company.drop.request"),
        "the request is on the trail: {trail}"
    );
}

fn cancelling(company: &str, by: &Signer, at: u64) -> Value {
    json!({
        "at": at,
        "signature": encoded(&by.stored(&statement(Act::Cancel, company, at))),
    })
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn the_platform_admin_confirms_and_the_company_waits_read_only_until_an_owner_cancels() {
    use crate::world::sealed_record;

    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let signer = Signer::new();
    let owner = signing_owner(&world, &cast, &signer).await;
    let a = cast.company_a.as_str();
    let base = format!("/api/companies/{a}/drop");
    let (status, body) = world
        .call(
            Method::POST,
            &format!("/api/companies/{}/drop/confirm", cast.company_b),
            Some(&cast.platform),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "nothing asked for B: {body}");
    let (status, body) = world
        .call(
            Method::POST,
            &base,
            Some(&owner),
            Some(asked(a, &signer, now())),
        )
        .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{body}");
    let confirm = format!("{base}/confirm");
    let (status, body) = world
        .call(Method::POST, &confirm, Some(&cast.owner_a), None)
        .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "an owner is not the second party: {body}"
    );
    let (status, body) = world
        .call(Method::POST, &confirm, Some(&cast.platform), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["state"], "scheduled", "{body}");
    let after = body["drop_after"].as_u64().expect("a time");
    assert!(
        after.abs_diff(now().saturating_add(604_800)) < 120,
        "seven days: {body}"
    );

    let records = format!("/api/companies/{a}/records");
    let (status, body) = world
        .call(Method::GET, &records, Some(&cast.member_a), None)
        .await;
    assert_eq!(status, StatusCode::OK, "reads go on: {body}");
    let (status, body) = world
        .call(
            Method::GET,
            &format!("/api/companies/{a}/keys/me"),
            Some(&cast.member_a),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "a member can still unlock: {body}");
    let (status, body) = world
        .call(
            Method::POST,
            &records,
            Some(&cast.member_a),
            Some(sealed_record()),
        )
        .await;
    assert_eq!(
        (status, body["error_code"].as_str()),
        (StatusCode::CONFLICT, Some("company_closing")),
        "{body}"
    );

    let cancel = format!("{base}/cancel");
    let (status, body) = world
        .call(
            Method::POST,
            &cancel,
            Some(&cast.admin_a),
            Some(cancelling(a, &signer, now())),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    let (status, body) = world
        .call(
            Method::POST,
            &cancel,
            Some(&owner),
            Some(asked(a, &signer, now())),
        )
        .await;
    assert_eq!(
        (status, body["error_code"].as_str()),
        (StatusCode::CONFLICT, Some("drop_unsigned")),
        "a request's signature does not cancel: {body}"
    );
    let (status, body) = world
        .call(
            Method::POST,
            &cancel,
            Some(&owner),
            Some(cancelling(a, &signer, now())),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["state"], "cancelled", "{body}");
    let (status, body) = world
        .call(
            Method::POST,
            &records,
            Some(&cast.member_a),
            Some(sealed_record()),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "writes resume: {body}");
    let (_, trail) = world
        .call(
            Method::GET,
            &format!("/api/companies/{a}/audit"),
            Some(&cast.owner_a),
            None,
        )
        .await;
    for action in ["company.drop.confirm", "company.drop.cancel"] {
        assert!(trail.to_string().contains(action), "{action} on the trail");
    }
}
