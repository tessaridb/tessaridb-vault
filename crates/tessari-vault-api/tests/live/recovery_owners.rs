//! An owner vouched after the recovery key was made has no wrap of it; an owner holding one gives them theirs, and only
//! to owners still waiting, each once.

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use crate::records::encoded;
use crate::world::{Cast, World};

fn path(cast: &Cast, tail: &str) -> String {
    format!("/api/companies/{}/recovery-key{tail}", cast.company_a)
}

fn wraps_to(keys: &[&str]) -> Value {
    let wraps: Vec<Value> = keys
        .iter()
        .map(|key| json!({ "key_id": key, "wrap": encoded(&[0xe3; 82]) }))
        .collect();
    json!({ "wraps": wraps })
}

async fn pending(world: &World, cast: &Cast) -> Vec<String> {
    let (status, listed) = world
        .call(
            Method::GET,
            &path(cast, "/pending"),
            Some(&cast.owner_a),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{listed}");
    listed["pending"]
        .as_array()
        .expect("a list")
        .iter()
        .map(|key| key["key_id"].as_str().expect("a key id").to_owned())
        .collect()
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn an_owner_gives_a_later_owner_the_recovery_key_once() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let (status, none) = world
        .call(
            Method::GET,
            &path(&cast, "/pending"),
            Some(&cast.owner_a),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "pending with no key: {none}");
    assert_eq!(none["error_code"], "not_found", "{none}");
    let key = json!({
        "key_id": "e1".repeat(16),
        "public": encoded(&[0xe1; 32]),
        "wraps": [{ "key_id": cast.owner_a_key, "wrap": encoded(&[0xe2; 82]) }],
    });
    let (status, body) = world
        .call(
            Method::POST,
            &path(&cast, ""),
            Some(&cast.owner_a),
            Some(key),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert!(pending(&world, &cast).await.is_empty());

    // Admin A becomes an owner: their key set now waits for a wrap, and they have none to read.
    let (status, body) = world
        .call(
            Method::PATCH,
            &format!(
                "/api/companies/{}/members/{}",
                cast.company_a, cast.admin_a_id
            ),
            Some(&cast.owner_a),
            Some(json!({ "role": "owner" })),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    assert_eq!(
        pending(&world, &cast).await,
        std::slice::from_ref(&cast.admin_a_key)
    );
    let (status, _) = world
        .call(
            Method::GET,
            &path(&cast, "/mine"),
            Some(&cast.admin_a),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let wraps_path = path(&cast, "/wraps");
    let give = |body: Value| world.call(Method::POST, &wraps_path, Some(&cast.owner_a), Some(body));
    for (refused, body) in [
        ("a member", wraps_to(&[&cast.member_a_key])),
        (
            "an owner already holding one",
            wraps_to(&[&cast.owner_a_key]),
        ),
        (
            "two to one",
            wraps_to(&[&cast.admin_a_key, &cast.admin_a_key]),
        ),
        ("none", wraps_to(&[])),
    ] {
        let (status, answer) = give(body).await;
        assert_eq!(status, StatusCode::CONFLICT, "{refused}: {answer}");
        assert_eq!(answer["error_code"], "outside_reach", "{refused}");
    }
    assert_eq!(
        pending(&world, &cast).await,
        std::slice::from_ref(&cast.admin_a_key)
    );

    let (status, body) = give(wraps_to(&[&cast.admin_a_key])).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    assert!(pending(&world, &cast).await.is_empty());
    let (status, mine) = world
        .call(
            Method::GET,
            &path(&cast, "/mine"),
            Some(&cast.admin_a),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{mine}");
    assert_eq!(mine["wrap"], encoded(&[0xe3; 82]));
    let (status, again) = give(wraps_to(&[&cast.admin_a_key])).await;
    assert_eq!(status, StatusCode::CONFLICT, "a second wrap: {again}");
    assert_eq!(again["error_code"], "outside_reach", "{again}");
}
