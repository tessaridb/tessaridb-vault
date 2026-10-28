//! Revocation: removing a share stops the member it reached at their next request, and marks the record so its next
//! revision is sealed under a new record key wrapped only to those who still reach it.

use std::time::Instant;

use axum::http::{Method, StatusCode};
use serde_json::Value;

use crate::shares::{listed, record_path, revision, shares, to_member};
use crate::world::{Cast, World};

async fn share_with_admin(world: &World, cast: &Cast, right: &str) {
    let (status, body) = world
        .call(
            Method::POST,
            &shares(cast, &cast.record_a),
            Some(&cast.member_a),
            Some(to_member(&cast.admin_a_id, &cast.admin_a_key, right, 9)),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
}

fn unshare(cast: &Cast, target: &str) -> String {
    format!("{}/{target}", shares(cast, &cast.record_a))
}

async fn rotation_due(world: &World, cast: &Cast) -> Value {
    let (status, list) = world
        .call(
            Method::GET,
            &format!("/api/companies/{}/records", cast.company_a),
            Some(&cast.member_a),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{list}");
    list["records"]
        .as_array()
        .expect("a list")
        .iter()
        .find(|record| record["id"] == cast.record_a.as_str())
        .expect("the author lists their record")["rotation_due"]
        .clone()
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn removing_a_share_stops_its_reader_at_once_and_the_next_revision_rotates_the_key() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    share_with_admin(&world, &cast, "read").await;
    let path = record_path(&cast, &cast.record_a);
    let (status, _) = world
        .call(Method::GET, &path, Some(&cast.admin_a), None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(rotation_due(&world, &cast).await, false);

    let (status, body) = world
        .call(
            Method::DELETE,
            &unshare(&cast, &cast.admin_a_id),
            Some(&cast.member_a),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let removed = Instant::now();
    let (status, _) = world
        .call(Method::GET, &path, Some(&cast.admin_a), None)
        .await;
    let refused_after = removed.elapsed();
    assert_eq!(status, StatusCode::NOT_FOUND, "a removed reader still read");
    println!("revocation: removed → first refused read in {refused_after:?}");
    assert!(
        !listed(&world, &cast, &cast.admin_a)
            .await
            .contains(&cast.record_a)
    );
    assert_eq!(rotation_due(&world, &cast).await, true);

    // A revision under the same record key keeps a key the removed reader may hold.
    let (status, refused) = world
        .call(
            Method::PUT,
            &path,
            Some(&cast.member_a),
            Some(revision(1, &cast.member_a_key, Vec::new())),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{refused}");
    assert_eq!(refused["error_code"], "rotation_due");
    let mut rotated = revision(1, &cast.member_a_key, Vec::new());
    rotated["key_id"] = serde_json::json!("e1".repeat(16));
    let (status, body) = world
        .call(Method::PUT, &path, Some(&cast.member_a), Some(rotated))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(rotation_due(&world, &cast).await, false);
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn only_an_editor_removes_a_share_and_only_one_that_exists() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    share_with_admin(&world, &cast, "read").await;
    let path = unshare(&cast, &cast.admin_a_id);
    let (status, _) = world
        .call(Method::DELETE, &path, Some(&cast.admin_a), None)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "a reader removed a share");
    let (status, _) = world
        .call(Method::DELETE, &path, Some(&cast.owner_a), None)
        .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "someone the record does not reach removed a share"
    );
    let (status, _) = world
        .call(
            Method::DELETE,
            &unshare(&cast, &cast.owner_a_id),
            Some(&cast.member_a),
            None,
        )
        .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "a share nobody made was removed"
    );
    let (status, _) = world
        .call(Method::DELETE, &path, Some(&cast.member_a), None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = world
        .call(Method::DELETE, &path, Some(&cast.member_a), None)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "a share was removed twice");
}
