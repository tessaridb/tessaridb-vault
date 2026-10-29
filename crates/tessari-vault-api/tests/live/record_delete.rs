//! Deleting a record: its author or a company admin removes it with every revision, wrap, share and file in one
//! transaction and the reason on the trail; anyone else is refused, or told it does not exist when they cannot reach it.

use axum::http::{Method, StatusCode};
use serde_json::Value as Json;
use tessari_vault_infrastructure::tessaridb::{Answer, Value};

use crate::records::create_by;
use crate::shares::{record_path, shares, to_member};
use crate::world::{Cast, World};

async fn delete(world: &World, path: &str, token: &str) -> (StatusCode, Json) {
    let headers = [
        ("authorization", format!("Bearer {token}")),
        ("vault-reason", "left the team".to_owned()),
    ];
    let (status, _, body) = world.call_with(Method::DELETE, path, &headers, None).await;
    (status, body)
}

/// What the store's last statement of `script` answered, read as the store's root.
async fn answered(world: &World, script: &str, record: &str) -> Value {
    let answers = world
        .database
        .run_at(
            &world.place,
            script,
            vec![("record".to_owned(), record.into())],
        )
        .await
        .expect("the store answers");
    match answers.last() {
        Some(Answer::Value { value, .. }) => value.clone(),
        other => panic!("expected a value, got {other:?}"),
    }
}

/// A file of `size` bytes on `record`, written whole by `token`.
async fn file(world: &World, cast: &Cast, record: &str, token: &str, size: usize) -> String {
    let (status, created) = world
        .call(
            Method::POST,
            &format!("{}/files", record_path(cast, record)),
            Some(token),
            Some(serde_json::json!({ "size": size })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let id = created["id"].as_str().expect("an id").to_owned();
    let (status, body) = world
        .put_octets(
            &format!("{}/files/{id}?offset=0", record_path(cast, record)),
            token,
            vec![7; size],
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    id
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn a_record_is_deleted_whole_by_its_author_or_an_admin_and_by_nobody_else() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let a = &cast.company_a;
    let mine = cast.record_a.clone();
    let small = file(&world, &cast, &mine, &cast.member_a, 1_000).await;
    let large = file(&world, &cast, &mine, &cast.member_a, 70_000).await;
    let owners = create_by(&world, a, &cast.owner_a, &"44".repeat(16), b"the owner's").await;
    let (status, body) = world
        .call(
            Method::POST,
            &shares(&cast, &owners),
            Some(&cast.owner_a),
            Some(to_member(&cast.member_a_id, &"22".repeat(16), "read", 3)),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let held = format!(
        "{:?}",
        answered(
            &world,
            &format!("INFO FOR VAULT records_{a} RECORDS;"),
            &mine
        )
        .await
    );
    assert!(
        held.contains(&format!("{mine}.1")) && held.contains(&format!("file.{small}")),
        "the listing shows what the delete must remove: {held}"
    );
    let part = answered(
        &world,
        "READ record_files:$record;",
        &format!("/{a}/{large}/0"),
    )
    .await;
    assert_ne!(
        part,
        Value::None,
        "the large file's part is in the bucket before"
    );

    let (status, body) = delete(&world, &record_path(&cast, &mine), &cast.admin_b).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "another company's admin: {body}"
    );
    let (status, body) = delete(&world, &record_path(&cast, &owners), &cast.member_a).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a reader who is no admin: {body}"
    );
    assert_eq!(body["error_code"], "forbidden", "{body}");
    let unshared = create_by(&world, a, &cast.owner_a, &"44".repeat(16), b"not shared").await;
    let (status, body) = delete(&world, &record_path(&cast, &unshared), &cast.member_a).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "a member it was never shared with learns nothing: {body}"
    );

    let (status, body) = delete(&world, &record_path(&cast, &mine), &cast.member_a).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "the author: {body}");
    let (status, body) = world
        .call(
            Method::GET,
            &record_path(&cast, &mine),
            Some(&cast.member_a),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    for table in [
        "records WHERE record_id",
        "revisions WHERE record",
        "shares WHERE record",
        "files WHERE record",
    ] {
        let left = answered(
            &world,
            &format!("RETURN array::len((SELECT * FROM {table} = $record));"),
            &mine,
        )
        .await;
        assert_eq!(left, Value::from(0_i64), "{table} still holds the record");
    }
    let entries = answered(
        &world,
        &format!("INFO FOR VAULT records_{a} RECORDS;"),
        &mine,
    )
    .await;
    for kept in [format!("{mine}.1"), format!("file.{small}")] {
        assert!(
            !format!("{entries:?}").contains(&kept),
            "{kept} is still in the vault: {entries:?}"
        );
    }
    let part = answered(
        &world,
        "READ record_files:$record;",
        &format!("/{a}/{large}/0"),
    )
    .await;
    assert_eq!(
        part,
        Value::None,
        "the large file's part is still in the bucket"
    );

    let shared = "RETURN array::len((SELECT * FROM shares WHERE record = $record));";
    assert_eq!(answered(&world, shared, &owners).await, Value::from(1_i64));
    let (status, body) = delete(&world, &record_path(&cast, &owners), &cast.admin_a).await;
    assert_eq!(
        status,
        StatusCode::NO_CONTENT,
        "an admin it was never shared with: {body}"
    );
    assert_eq!(
        answered(&world, shared, &owners).await,
        Value::from(0_i64),
        "the share went with its record"
    );
    let (status, body) = delete(&world, &record_path(&cast, &owners), &cast.admin_a).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "a second delete: {body}");

    let (status, trail) = world
        .call(
            Method::GET,
            &format!("/api/companies/{a}/audit"),
            Some(&cast.owner_a),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{trail}");
    let deletes: Vec<(String, String)> = trail["events"]
        .as_array()
        .expect("events")
        .iter()
        .filter(|event| event["action"] == "record.delete")
        .map(|event| {
            let text = |name: &str| event[name].as_str().expect(name).to_owned();
            (text("target"), text("reason"))
        })
        .collect();
    assert_eq!(
        deletes,
        [
            (owners.clone(), "left the team".to_owned()),
            (mine.clone(), "left the team".to_owned())
        ],
        "newest first, each with its reason"
    );
}
