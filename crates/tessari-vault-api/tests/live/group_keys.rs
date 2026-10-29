//! Group keys: an admin keys a company group, department or team, and its private key reaches, wrapped, exactly the
//! members inside the group's reach who keep a key set; nothing is wrapped to anyone outside it.

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use crate::records::encoded;
use crate::world::{Cast, World};

fn wrap_to(key_id: &str, seed: u8) -> Value {
    json!({ "key_id": key_id, "wrap": encoded(&[seed; 82]) })
}

fn group_key(key_id: &str, wraps: Vec<Value>) -> Value {
    json!({ "key_id": key_id, "public": encoded(&[0x5a; 32]), "wraps": wraps })
}

fn key_path(cast: &Cast, group: &str) -> String {
    format!("/api/companies/{}/groups/{group}/key", cast.company_a)
}

/// Put member A into team A, as an admin does.
async fn member_in_team(world: &World, cast: &Cast, account: &str) {
    let (status, body) = world
        .call(
            Method::POST,
            &format!(
                "/api/companies/{}/groups/{}/members",
                cast.company_a, cast.team_a
            ),
            Some(&cast.admin_a),
            Some(json!({ "account_id": account })),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
}

async fn pending(world: &World, cast: &Cast, group: &str) -> Vec<String> {
    let (status, body) = world
        .call(
            Method::GET,
            &format!("{}/pending", key_path(cast, group)),
            Some(&cast.admin_a),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let mut keys: Vec<String> = body["pending"]
        .as_array()
        .expect("a list")
        .iter()
        .map(|entry| entry["key_id"].as_str().expect("a key id").to_owned())
        .collect();
    keys.sort();
    keys
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn an_admin_keys_a_team_and_each_member_reads_only_their_own_wrap() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    member_in_team(&world, &cast, &cast.member_a_id).await;
    let path = key_path(&cast, &cast.team_a);
    let body = group_key(&"c1".repeat(16), vec![wrap_to(&cast.member_a_key, 1)]);

    let (status, refused) = world
        .call(Method::PUT, &path, Some(&cast.member_a), Some(body.clone()))
        .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a member keyed a team: {refused}"
    );
    let (status, kept) = world
        .call(Method::PUT, &path, Some(&cast.admin_a), Some(body.clone()))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{kept}");
    let (status, twice) = world
        .call(Method::PUT, &path, Some(&cast.admin_a), Some(body.clone()))
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{twice}");
    assert_eq!(twice["error_code"], "keys_exist");

    let (status, read) = world
        .call(Method::GET, &path, Some(&cast.member_a), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{read}");
    assert_eq!(read["key_id"], "c1".repeat(16));
    assert_eq!(read["public"], encoded(&[0x5a; 32]));
    assert_eq!(read["wrap"], encoded(&[1; 82]));
    // The admin is not in the team and holds no wrap of its key; nobody is answered another member's.
    let (status, read) = world
        .call(Method::GET, &path, Some(&cast.admin_a), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{read}");
    assert_eq!(read["wrap"], Value::Null);
    let (status, _) = world
        .call(Method::GET, &path, Some(&cast.admin_b), None)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn a_group_key_is_wrapped_only_to_keys_inside_its_reach() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    member_in_team(&world, &cast, &cast.member_a_id).await;
    for (group, outsider) in [
        (&cast.team_a, cast.owner_a_key.clone()),
        (&cast.team_a, "ee".repeat(16)),
        (&cast.department_a, cast.admin_a_key.clone()),
    ] {
        let (status, refused) = world
            .call(
                Method::PUT,
                &key_path(&cast, group),
                Some(&cast.admin_a),
                Some(group_key(&"c2".repeat(16), vec![wrap_to(&outsider, 2)])),
            )
            .await;
        assert_eq!(
            status,
            StatusCode::CONFLICT,
            "{group} wrapped to {outsider}: {refused}"
        );
        assert_eq!(refused["error_code"], "outside_reach");
    }
    // A team member is inside the team's department; every member keeping keys is inside the company.
    let (status, body) = world
        .call(
            Method::PUT,
            &key_path(&cast, &cast.department_a),
            Some(&cast.admin_a),
            Some(group_key(
                &"c3".repeat(16),
                vec![wrap_to(&cast.member_a_key, 3)],
            )),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let everyone = [&cast.owner_a_key, &cast.admin_a_key, &cast.member_a_key];
    let (status, body) = world
        .call(
            Method::PUT,
            &key_path(&cast, &cast.company_a),
            Some(&cast.owner_a),
            Some(group_key(
                &"c4".repeat(16),
                everyone.iter().map(|key| wrap_to(key, 4)).collect(),
            )),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert!(pending(&world, &cast, &cast.company_a).await.is_empty());
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn the_pending_list_names_the_reach_still_without_a_wrap() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    member_in_team(&world, &cast, &cast.member_a_id).await;
    let path = key_path(&cast, &cast.team_a);
    let (status, body) = world
        .call(
            Method::PUT,
            &path,
            Some(&cast.admin_a),
            Some(group_key(&"c5".repeat(16), Vec::new())),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(
        pending(&world, &cast, &cast.team_a).await,
        vec![cast.member_a_key.clone()]
    );
    let wraps = format!("{path}/wraps");
    let one = json!({ "wraps": [wrap_to(&cast.member_a_key, 5)] });
    let (status, body) = world
        .call(Method::POST, &wraps, Some(&cast.admin_a), Some(one.clone()))
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    assert!(pending(&world, &cast, &cast.team_a).await.is_empty());
    let (status, twice) = world
        .call(Method::POST, &wraps, Some(&cast.admin_a), Some(one))
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "a key wrapped twice: {twice}");
    assert_eq!(twice["error_code"], "outside_reach");
    member_in_team(&world, &cast, &cast.owner_a_id).await;
    assert_eq!(
        pending(&world, &cast, &cast.team_a).await,
        vec![cast.owner_a_key.clone()]
    );
    let (status, _) = world
        .call(
            Method::GET,
            &format!("{path}/pending"),
            Some(&cast.member_a),
            None,
        )
        .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a member read the pending list"
    );
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn after_a_member_leaves_the_pending_list_names_everyone_the_new_key_is_for() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    member_in_team(&world, &cast, &cast.member_a_id).await;
    member_in_team(&world, &cast, &cast.owner_a_id).await;
    let path = key_path(&cast, &cast.team_a);
    let wraps = vec![
        wrap_to(&cast.member_a_key, 1),
        wrap_to(&cast.owner_a_key, 2),
    ];
    let (status, body) = world
        .call(
            Method::PUT,
            &path,
            Some(&cast.admin_a),
            Some(group_key(&"c6".repeat(16), wraps)),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert!(pending(&world, &cast, &cast.team_a).await.is_empty());
    let (status, body) = world
        .call(
            Method::DELETE,
            &format!(
                "/api/companies/{}/groups/{}/members/{}",
                cast.company_a, cast.team_a, cast.owner_a_id
            ),
            Some(&cast.admin_a),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    // The key the leaver held is retired, so the new one is owed to everyone still in the team.
    assert_eq!(
        pending(&world, &cast, &cast.team_a).await,
        vec![cast.member_a_key.clone()]
    );
    let (status, body) = world
        .call(
            Method::PUT,
            &path,
            Some(&cast.admin_a),
            Some(group_key(
                &"c7".repeat(16),
                vec![wrap_to(&cast.member_a_key, 3)],
            )),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "the new key: {body}");
}
