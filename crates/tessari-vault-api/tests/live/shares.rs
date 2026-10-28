//! Sharing: a record reaches the members of a team, a department, the whole company or one member it is shared with,
//! each through a wrap of its key to a key they hold; nobody else finds it, and a revision keeps every share's wrap.

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use crate::records::encoded;
use crate::world::{Cast, World, sealed_record};

/// Put `account` into team A, as an admin does.
pub(crate) async fn into_team(world: &World, cast: &Cast, account: &str) {
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

/// Key `group` with `key_id`, wrapped to `members`.
pub(crate) async fn keyed(world: &World, cast: &Cast, group: &str, key_id: &str, members: &[&str]) {
    let wraps: Vec<Value> = members
        .iter()
        .map(|member| json!({ "key_id": member, "wrap": encoded(&[0x31; 82]) }))
        .collect();
    let (status, body) = world
        .call(
            Method::PUT,
            &format!("/api/companies/{}/groups/{group}/key", cast.company_a),
            Some(&cast.owner_a),
            Some(json!({ "key_id": key_id, "public": encoded(&[0x41; 32]), "wraps": wraps })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
}

fn shares(cast: &Cast, record: &str) -> String {
    format!("/api/companies/{}/records/{record}/shares", cast.company_a)
}

fn record_path(cast: &Cast, record: &str) -> String {
    format!("/api/companies/{}/records/{record}", cast.company_a)
}

fn to_group(group: &str, key_id: &str, right: &str, seed: u8) -> Value {
    json!({ "group": group, "right": right, "key_id": key_id, "wrap": encoded(&[seed; 82]) })
}

fn to_member(account: &str, key_id: &str, right: &str, seed: u8) -> Value {
    json!({ "member": account, "right": right, "key_id": key_id, "wrap": encoded(&[seed; 82]) })
}

async fn listed(world: &World, cast: &Cast, token: &str) -> Vec<String> {
    let (status, list) = world
        .call(
            Method::GET,
            &format!("/api/companies/{}/records", cast.company_a),
            Some(token),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{list}");
    list["records"]
        .as_array()
        .expect("a list")
        .iter()
        .map(|record| record["id"].as_str().expect("an id").to_owned())
        .collect()
}

/// A revision of the record over `expected` by the member whose key is `key`, carrying `wraps`.
fn revision(expected: i64, key: &str, wraps: Vec<Value>) -> Value {
    let mut body = sealed_record();
    body.as_object_mut()
        .map(|fields| fields.remove("record_type"));
    body["revision"] = json!(expected);
    body["author_key"] = json!(key);
    body["wraps"] = json!(wraps);
    body
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn a_record_shared_with_a_team_reaches_its_members_and_nobody_else() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    into_team(&world, &cast, &cast.admin_a_id).await;
    let team_key = "d1".repeat(16);
    keyed(&world, &cast, &cast.team_a, &team_key, &[&cast.admin_a_key]).await;
    let path = record_path(&cast, &cast.record_a);

    let (status, _) = world
        .call(Method::GET, &path, Some(&cast.admin_a), None)
        .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "a record nobody shared was read"
    );
    let (status, body) = world
        .call(
            Method::POST,
            &shares(&cast, &cast.record_a),
            Some(&cast.member_a),
            Some(to_group(&cast.team_a, &team_key, "read", 7)),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    let (status, read) = world
        .call(Method::GET, &path, Some(&cast.admin_a), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{read}");
    assert_eq!(read["recipient"], format!("group:{team_key}"));
    assert_eq!(read["wrap"], encoded(&[7; 82]));
    assert!(
        listed(&world, &cast, &cast.admin_a)
            .await
            .contains(&cast.record_a)
    );
    // The author still opens it under their own key.
    let (_, own) = world
        .call(Method::GET, &path, Some(&cast.member_a), None)
        .await;
    assert_eq!(own["recipient"], format!("member:{}", cast.member_a_key));

    // The owner is in the company, not the team.
    let (status, _) = world
        .call(Method::GET, &path, Some(&cast.owner_a), None)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(
        !listed(&world, &cast, &cast.owner_a)
            .await
            .contains(&cast.record_a)
    );
    let (status, _) = world
        .call(Method::GET, &path, Some(&cast.admin_b), None)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // A reader does not revise and does not share on.
    let (status, refused) = world
        .call(
            Method::PUT,
            &path,
            Some(&cast.admin_a),
            Some(revision(1, &cast.admin_a_key, Vec::new())),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "a reader revised: {refused}");
    let (status, refused) = world
        .call(
            Method::POST,
            &shares(&cast, &cast.record_a),
            Some(&cast.admin_a),
            Some(to_member(&cast.owner_a_id, &cast.owner_a_key, "read", 8)),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a reader shared on: {refused}"
    );
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn a_record_shared_with_the_department_the_company_or_one_member_reaches_them() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    // A team member is in the team's department.
    into_team(&world, &cast, &cast.admin_a_id).await;
    let department_key = "d2".repeat(16);
    keyed(
        &world,
        &cast,
        &cast.department_a,
        &department_key,
        &[&cast.admin_a_key],
    )
    .await;
    let (status, body) = world
        .call(
            Method::POST,
            &shares(&cast, &cast.record_a),
            Some(&cast.member_a),
            Some(to_group(&cast.department_a, &department_key, "read", 2)),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let (_, read) = world
        .call(
            Method::GET,
            &record_path(&cast, &cast.record_a),
            Some(&cast.admin_a),
            None,
        )
        .await;
    assert_eq!(read["recipient"], format!("group:{department_key}"));

    let company_key = "d3".repeat(16);
    keyed(
        &world,
        &cast,
        &cast.company_a,
        &company_key,
        &[&cast.owner_a_key, &cast.admin_a_key, &cast.member_a_key],
    )
    .await;
    let other =
        crate::records::create(&world, &cast.company_a, &cast.member_a, b"for everyone").await;
    let (status, body) = world
        .call(
            Method::POST,
            &shares(&cast, &other),
            Some(&cast.member_a),
            Some(to_group(&cast.company_a, &company_key, "read", 3)),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let (status, read) = world
        .call(
            Method::GET,
            &record_path(&cast, &other),
            Some(&cast.owner_a),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{read}");
    assert_eq!(read["recipient"], format!("group:{company_key}"));

    let direct =
        crate::records::create(&world, &cast.company_a, &cast.member_a, b"for the owner").await;
    let (status, body) = world
        .call(
            Method::POST,
            &shares(&cast, &direct),
            Some(&cast.member_a),
            Some(to_member(&cast.owner_a_id, &cast.owner_a_key, "read", 4)),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let (_, read) = world
        .call(
            Method::GET,
            &record_path(&cast, &direct),
            Some(&cast.owner_a),
            None,
        )
        .await;
    assert_eq!(read["recipient"], format!("member:{}", cast.owner_a_key));
    assert_eq!(read["wrap"], encoded(&[4; 82]));
    let (status, _) = world
        .call(
            Method::GET,
            &record_path(&cast, &direct),
            Some(&cast.admin_a),
            None,
        )
        .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "a member share reached someone else"
    );
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn a_share_names_a_key_its_target_holds_and_is_made_once() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let path = shares(&cast, &cast.record_a);
    let post = |token: String, body: Value| {
        let world = &world;
        let path = path.clone();
        async move {
            world
                .call(Method::POST, &path, Some(&token), Some(body))
                .await
        }
    };
    let (status, _) = post(
        cast.owner_a.clone(),
        to_member(&cast.admin_a_id, &cast.admin_a_key, "read", 1),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "someone who cannot reach the record shared it"
    );
    let (status, unkeyed) = post(
        cast.member_a.clone(),
        to_group(&cast.team_a, &"d4".repeat(16), "read", 1),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{unkeyed}");
    assert_eq!(unkeyed["error_code"], "no_key");
    let (status, wrong) = post(
        cast.member_a.clone(),
        to_member(&cast.admin_a_id, &cast.owner_a_key, "read", 1),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{wrong}");
    assert_eq!(wrong["error_code"], "unknown_key");
    let (status, _) = post(
        cast.member_a.clone(),
        to_member(&cast.admin_b_id, &cast.owner_a_key, "read", 1),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "a share reached another company's member"
    );
    let (status, body) = post(
        cast.member_a.clone(),
        to_member(&cast.admin_a_id, &cast.admin_a_key, "read", 1),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let (status, twice) = post(
        cast.member_a.clone(),
        to_member(&cast.admin_a_id, &cast.admin_a_key, "edit", 2),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{twice}");
    assert_eq!(twice["error_code"], "already_shared");
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn a_revision_keeps_a_wrap_for_every_share_and_an_editor_may_write_one() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let (status, body) = world
        .call(
            Method::POST,
            &shares(&cast, &cast.record_a),
            Some(&cast.member_a),
            Some(to_member(&cast.admin_a_id, &cast.admin_a_key, "edit", 5)),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let path = record_path(&cast, &cast.record_a);
    let admin = format!("member:{}", cast.admin_a_key);
    let author = format!("member:{}", cast.member_a_key);

    let (status, stale) = world
        .call(
            Method::PUT,
            &path,
            Some(&cast.member_a),
            Some(revision(1, &cast.member_a_key, Vec::new())),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "a revision dropped a share's wrap: {stale}"
    );
    assert_eq!(stale["error_code"], "stale_shares");
    let extra =
        json!({ "recipient": format!("member:{}", cast.owner_a_key), "wrap": encoded(&[6; 82]) });
    let kept = json!({ "recipient": admin, "wrap": encoded(&[6; 82]) });
    let (status, stale) = world
        .call(
            Method::PUT,
            &path,
            Some(&cast.member_a),
            Some(revision(1, &cast.member_a_key, vec![kept.clone(), extra])),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "a revision added a reader: {stale}"
    );
    let (status, revised) = world
        .call(
            Method::PUT,
            &path,
            Some(&cast.member_a),
            Some(revision(1, &cast.member_a_key, vec![kept])),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{revised}");
    let (_, read) = world
        .call(Method::GET, &path, Some(&cast.admin_a), None)
        .await;
    assert_eq!(read["record"]["revision"], 2);
    assert_eq!(read["wrap"], encoded(&[6; 82]));

    // The editor writes the next one, keeping the author's wrap; their own goes in the body.
    let keep_author = json!({ "recipient": author, "wrap": encoded(&[7; 82]) });
    let (status, revised) = world
        .call(
            Method::PUT,
            &path,
            Some(&cast.admin_a),
            Some(revision(2, &cast.admin_a_key, vec![keep_author])),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{revised}");
    let (_, read) = world
        .call(Method::GET, &path, Some(&cast.member_a), None)
        .await;
    assert_eq!(read["record"]["revision"], 3);
    assert_eq!(read["wrap"], encoded(&[7; 82]));
    assert_eq!(read["author_key"], cast.admin_a_key.as_str());
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn a_shared_records_files_are_read_by_its_readers_and_added_by_its_editors() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    for (account, key, right, seed) in [
        (&cast.admin_a_id, &cast.admin_a_key, "read", 1),
        (&cast.owner_a_id, &cast.owner_a_key, "edit", 2),
    ] {
        let (status, body) = world
            .call(
                Method::POST,
                &shares(&cast, &cast.record_a),
                Some(&cast.member_a),
                Some(to_member(account, key, right, seed)),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
    }
    let files = format!("{}/files", record_path(&cast, &cast.record_a));
    let (status, bytes) = crate::files::get_part(
        &world,
        &format!("{files}/{}?offset=0&limit=6", cast.file_a),
        &cast.admin_a,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a reader could not read the record's file"
    );
    assert_eq!(bytes, b"sealed");
    let (status, refused) = world
        .call(
            Method::POST,
            &files,
            Some(&cast.admin_a),
            Some(json!({ "size": 10 })),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a reader added a file: {refused}"
    );
    let (status, created) = world
        .call(
            Method::POST,
            &files,
            Some(&cast.owner_a),
            Some(json!({ "size": 10 })),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "an editor could not add a file: {created}"
    );
}
