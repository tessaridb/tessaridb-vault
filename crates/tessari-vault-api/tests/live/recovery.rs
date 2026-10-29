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
