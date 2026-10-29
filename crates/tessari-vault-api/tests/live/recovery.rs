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

fn requests_path(cast: &Cast, tail: &str) -> String {
    format!("/api/companies/{}/recovery-requests{tail}", cast.company_a)
}

/// Owner A keys recovery, member A escrows and asks to be recovered; the request's id.
async fn requested(world: &World, cast: &Cast) -> String {
    keyed(world, cast).await;
    let escrow = json!({ "recovery_key_id": "e1".repeat(16), "wrap": encoded(&[0xe3; 160]) });
    let (status, body) = world
        .call(
            Method::PUT,
            &escrow_path(cast, "me"),
            Some(&cast.member_a),
            Some(escrow),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let (status, asked) = world
        .call(
            Method::POST,
            &requests_path(cast, ""),
            Some(&cast.member_a),
            Some(json!({ "handoff_public": encoded(&[0xe4; 32]) })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{asked}");
    asked["id"].as_str().expect("an id").to_owned()
}

fn handoff() -> Value {
    json!({ "wrap": encoded(&[0xe5; 210]) })
}

fn rewrap(request: &str) -> Value {
    json!({
        "request": request,
        "kdf": { "salt": encoded(&[0xe6; 16]), "memory_kib": 47_104, "passes": 2, "lanes": 1 },
        "wrapped": encoded(&[0xe7; 120]),
    })
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn an_owner_recovers_a_member_who_takes_their_keys_back_once() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let request = requested(&world, &cast).await;
    let (status, open) = world
        .call(
            Method::GET,
            &requests_path(&cast, ""),
            Some(&cast.owner_a),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{open}");
    assert_eq!(open["requests"][0]["id"], request.as_str());
    assert_eq!(open["requests"][0]["account"], cast.member_a_id.as_str());
    assert_eq!(open["requests"][0]["handoff_public"], encoded(&[0xe4; 32]));

    let (status, body) = world
        .call(
            Method::POST,
            &requests_path(&cast, &format!("/{request}/handoff")),
            Some(&cast.owner_a),
            Some(handoff()),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let redeem = requests_path(&cast, &format!("/{request}/redeem"));
    let (status, taken) = world
        .call(Method::POST, &redeem, Some(&cast.member_a), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{taken}");
    assert_eq!(taken["wrap"], encoded(&[0xe5; 210]));
    let (status, again) = world
        .call(Method::POST, &redeem, Some(&cast.member_a), None)
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{again}");
    assert_eq!(again["error_code"], "recovery_closed");

    let keys_me = format!("/api/companies/{}/keys/me", cast.company_a);
    let rewrap_path = format!("/api/companies/{}/keys/me/rewrap", cast.company_a);
    let (status, body) = world
        .call(
            Method::POST,
            &rewrap_path,
            Some(&cast.member_a),
            Some(rewrap(&request)),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let (status, mine) = world
        .call(Method::GET, &keys_me, Some(&cast.member_a), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{mine}");
    assert_eq!(mine["kdf"]["salt"], encoded(&[0xe6; 16]));
    assert_eq!(mine["wrapped"], encoded(&[0xe7; 120]));
    assert_eq!(
        mine["key_id"],
        cast.member_a_key.as_str(),
        "the same keys, newly wrapped"
    );
    let (status, again) = world
        .call(
            Method::POST,
            &rewrap_path,
            Some(&cast.member_a),
            Some(rewrap(&request)),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{again}");
    assert_eq!(again["error_code"], "recovery_closed");

    let seen = notices(&world, &cast, &cast.member_a).await;
    let kinds: Vec<&str> = seen
        .iter()
        .filter_map(|notice| notice["kind"].as_str())
        .collect();
    assert_eq!(kinds, ["recovery"], "{seen:?}");
    assert_eq!(seen[0]["by"], cast.owner_a_id.as_str());
    let (status, trail) = world
        .call(
            Method::GET,
            &format!("/api/companies/{}/audit", cast.company_a),
            Some(&cast.owner_a),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{trail}");
    let trail = trail.to_string();
    for action in [
        "member.recovery.request",
        "member.recover",
        "member.recovery.replay",
        "keys.rewrap",
    ] {
        assert!(trail.contains(action), "{action} missing from the trail");
    }
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn nobody_but_an_owner_hands_over_and_nobody_but_the_member_redeems() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let request = requested(&world, &cast).await;
    let handoff_path = requests_path(&cast, &format!("/{request}/handoff"));
    let redeem = requests_path(&cast, &format!("/{request}/redeem"));
    let (status, early) = world
        .call(Method::POST, &redeem, Some(&cast.member_a), None)
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{early}");
    assert_eq!(early["error_code"], "not_handed_over");
    for (caller, expected) in [
        (&cast.admin_a, StatusCode::FORBIDDEN),
        (&cast.member_a, StatusCode::FORBIDDEN),
        (&cast.admin_b, StatusCode::NOT_FOUND),
        (&cast.platform, StatusCode::NOT_FOUND),
    ] {
        let (status, refused) = world
            .call(Method::POST, &handoff_path, Some(caller), Some(handoff()))
            .await;
        assert_eq!(status, expected, "{refused}");
        let (status, refused) = world
            .call(Method::GET, &requests_path(&cast, ""), Some(caller), None)
            .await;
        assert_eq!(status, expected, "{refused}");
    }
    // The operator, holding the store's root account, reads only a sealed vault where the escrow is kept.
    let refused = world
        .database
        .run_at(
            &world.place,
            &format!(
                "REVEAL body FROM records_{}:'escrow.{}';",
                cast.company_a, cast.member_a_id
            ),
            Vec::new(),
        )
        .await
        .expect_err("the operator opened a member's escrow");
    assert_eq!(refused.to_string(), "the vault is sealed");

    let (status, body) = world
        .call(
            Method::POST,
            &handoff_path,
            Some(&cast.owner_a),
            Some(handoff()),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    for (caller, expected) in [
        (&cast.admin_a, StatusCode::NOT_FOUND),
        (&cast.owner_a, StatusCode::NOT_FOUND),
        (&cast.admin_b, StatusCode::NOT_FOUND),
    ] {
        let (status, refused) = world.call(Method::POST, &redeem, Some(caller), None).await;
        assert_eq!(status, expected, "{refused}");
    }
    // A request past its 72 hours is closed: the row's own expiry, moved back, is what the redemption reads.
    world
        .database
        .run_at(
            &world.place,
            "UPDATE recovery_requests:$id SET expires = 0;",
            vec![(
                "id".to_owned(),
                tessari_vault_infrastructure::tessaridb::Value::String(request.clone()),
            )],
        )
        .await
        .expect("the row moves");
    let (status, late) = world
        .call(Method::POST, &redeem, Some(&cast.member_a), None)
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{late}");
    assert_eq!(late["error_code"], "recovery_closed");
    let (status, refused) = world
        .call(
            Method::POST,
            &format!("/api/companies/{}/keys/me/rewrap", cast.company_a),
            Some(&cast.member_a),
            Some(rewrap(&request)),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{refused}");
    assert_eq!(refused["error_code"], "recovery_closed");
}
