//! Organisation recovery (C7): the company's recovery key, kept by its owners. Only an owner keys it, once, and only to
//! owners' key sets; every member reads its public half, and each owner reads back only their own wrap of it.

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use crate::records::encoded;
use crate::world::{Cast, World};

fn recovery_key(wrap_to: &str) -> Value {
    json!({
        "key_id": "e1".repeat(16),
        "public": encoded(&[0xe1; 32]),
        "wraps": [{ "key_id": wrap_to, "wrap": encoded(&[0xe2; 82]) }],
    })
}

fn path(cast: &Cast, tail: &str) -> String {
    format!("/api/companies/{}/recovery-key{tail}", cast.company_a)
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn owners_keep_the_recovery_key_and_members_read_its_public_half() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let (status, body) = world
        .call(
            Method::POST,
            &path(&cast, ""),
            Some(&cast.owner_a),
            Some(recovery_key(&cast.owner_a_key)),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    let (status, public) = world
        .call(Method::GET, &path(&cast, ""), Some(&cast.member_a), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{public}");
    assert_eq!(public["key_id"], "e1".repeat(16));
    assert_eq!(public["public"], encoded(&[0xe1; 32]));
    assert_eq!(public.get("wraps"), None, "{public}");

    let (status, mine) = world
        .call(
            Method::GET,
            &path(&cast, "/mine"),
            Some(&cast.owner_a),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{mine}");
    assert_eq!(mine["key_id"], "e1".repeat(16));
    assert_eq!(mine["wrap"], encoded(&[0xe2; 82]));
    for lower in [&cast.admin_a, &cast.member_a] {
        let (status, refused) = world
            .call(Method::GET, &path(&cast, "/mine"), Some(lower), None)
            .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{refused}");
    }
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn only_an_owner_keys_recovery_to_owners_and_only_once() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    for (caller, wrap_to, expected) in [
        (&cast.admin_a, &cast.admin_a_key, StatusCode::FORBIDDEN),
        (&cast.member_a, &cast.member_a_key, StatusCode::FORBIDDEN),
        (&cast.admin_b, &cast.owner_a_key, StatusCode::NOT_FOUND),
    ] {
        let (status, refused) = world
            .call(
                Method::POST,
                &path(&cast, ""),
                Some(caller),
                Some(recovery_key(wrap_to)),
            )
            .await;
        assert_eq!(status, expected, "{refused}");
    }
    // An owner may not wrap the recovery key to anyone but an owner: an admin holding it would reach every member.
    let (status, refused) = world
        .call(
            Method::POST,
            &path(&cast, ""),
            Some(&cast.owner_a),
            Some(recovery_key(&cast.admin_a_key)),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{refused}");
    assert_eq!(refused["error_code"], "outside_reach");
    let (status, none) = world
        .call(Method::GET, &path(&cast, ""), Some(&cast.member_a), None)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{none}");

    let (status, body) = world
        .call(
            Method::POST,
            &path(&cast, ""),
            Some(&cast.owner_a),
            Some(recovery_key(&cast.owner_a_key)),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let (status, again) = world
        .call(
            Method::POST,
            &path(&cast, ""),
            Some(&cast.owner_a),
            Some(recovery_key(&cast.owner_a_key)),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{again}");
    assert_eq!(again["error_code"], "recovery_keyed");
}

/// Owner A keys the recovery key `e1e1...`, so members can escrow to it.
async fn keyed(world: &World, cast: &Cast) {
    let (status, body) = world
        .call(
            Method::POST,
            &path(cast, ""),
            Some(&cast.owner_a),
            Some(recovery_key(&cast.owner_a_key)),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
}

fn escrow_path(cast: &Cast, account: &str) -> String {
    format!("/api/companies/{}/keys/{account}/escrow", cast.company_a)
}

fn notices_path(cast: &Cast) -> String {
    format!("/api/companies/{}/notices/me", cast.company_a)
}

async fn notices(world: &World, cast: &Cast, caller: &str) -> Vec<Value> {
    let (status, body) = world
        .call(Method::GET, &notices_path(cast), Some(caller), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["notices"].as_array().expect("a list").clone()
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn an_owner_reading_a_members_escrow_leaves_that_member_a_notice() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    keyed(&world, &cast).await;
    let escrow = json!({ "recovery_key_id": "e1".repeat(16), "wrap": encoded(&[0xe3; 160]) });
    let (status, body) = world
        .call(
            Method::PUT,
            &escrow_path(&cast, "me"),
            Some(&cast.member_a),
            Some(escrow),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    assert!(notices(&world, &cast, &cast.member_a).await.is_empty());

    let (status, read) = world
        .call(
            Method::GET,
            &escrow_path(&cast, &cast.member_a_id),
            Some(&cast.owner_a),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{read}");
    assert_eq!(read["recovery_key_id"], "e1".repeat(16));
    assert_eq!(read["wrap"], encoded(&[0xe3; 160]));

    let seen = notices(&world, &cast, &cast.member_a).await;
    assert_eq!(seen.len(), 1, "{seen:?}");
    assert_eq!(seen[0]["kind"], "escrow.read");
    assert_eq!(seen[0]["by"], cast.owner_a_id.as_str());
    // A notice is for the member it is about, and nobody else reads it.
    assert!(notices(&world, &cast, &cast.admin_a).await.is_empty());
    assert!(notices(&world, &cast, &cast.owner_a).await.is_empty());
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn only_an_owner_reads_an_escrow_and_only_to_the_standing_key() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let escrow = |key: &str| json!({ "recovery_key_id": key, "wrap": encoded(&[0xe3; 160]) });
    // Nothing to escrow to before an owner keys the company.
    let (status, refused) = world
        .call(
            Method::PUT,
            &escrow_path(&cast, "me"),
            Some(&cast.member_a),
            Some(escrow(&"e1".repeat(16))),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{refused}");
    keyed(&world, &cast).await;
    let (status, refused) = world
        .call(
            Method::PUT,
            &escrow_path(&cast, "me"),
            Some(&cast.member_a),
            Some(escrow(&"f1".repeat(16))),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{refused}");
    assert_eq!(refused["error_code"], "unknown_key");
    let (status, body) = world
        .call(
            Method::PUT,
            &escrow_path(&cast, "me"),
            Some(&cast.member_a),
            Some(escrow(&"e1".repeat(16))),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");

    for (caller, expected) in [
        (&cast.admin_a, StatusCode::FORBIDDEN),
        (&cast.member_a, StatusCode::FORBIDDEN),
        (&cast.admin_b, StatusCode::NOT_FOUND),
        (&cast.platform, StatusCode::NOT_FOUND),
    ] {
        let (status, refused) = world
            .call(
                Method::GET,
                &escrow_path(&cast, &cast.member_a_id),
                Some(caller),
                None,
            )
            .await;
        assert_eq!(status, expected, "{refused}");
    }
    // A refused read is no read: the member is told of none.
    assert!(notices(&world, &cast, &cast.member_a).await.is_empty());
}
