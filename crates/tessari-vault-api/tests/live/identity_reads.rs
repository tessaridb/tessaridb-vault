//! A row the vault reads by its id is read by its address, so no statistic the planner keeps can turn the read into
//! another index's and have it refused: statistics taken while a company had no rows estimate its first ones at zero.

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use crate::world::{Cast, World, sealed_record_by};

async fn created(world: &World, path: &str, token: &str, body: Value) -> String {
    let (status, answer) = world
        .call(Method::POST, path, Some(token), Some(body))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{path}: {answer}");
    answer["id"].as_str().expect("an id").to_owned()
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn rows_are_read_by_id_after_statistics_taken_before_their_company_had_any() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let (b, admin) = (&cast.company_b, &cast.admin_b);
    let key = "66".repeat(16);
    let (status, body) = world
        .call(
            Method::PUT,
            &format!("/api/companies/{b}/keys/me"),
            Some(admin),
            Some(crate::keys::key_set(&key, 6)),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    world
        .database
        .run_at(
            &world.place,
            "ANALYZE TABLE records; ANALYZE TABLE groups; ANALYZE TABLE files; ANALYZE TABLE record_types;",
            Vec::new(),
        )
        .await
        .expect("statistics taken");

    // Every company has a group of its own, so only a company the statistics never saw has none counted.
    let (c, owner_c) =
        crate::world::company(&world, &cast.platform, "Gamma", "owner@gamma.test").await;
    let owner_c = world.token(&owner_c).await;
    let group = created(
        &world,
        &format!("/api/companies/{c}/groups"),
        &owner_c,
        json!({ "kind": "department", "name": "Finance" }),
    )
    .await;
    let path = format!("/api/companies/{c}/groups/{group}/members");
    let (status, read) = world.call(Method::GET, &path, Some(&owner_c), None).await;
    assert_eq!(status, StatusCode::OK, "{path}: {read}");

    let mut definition = sealed_record_by(&key);
    definition["record_type"] = json!("record-type");
    let definition = created(
        &world,
        &format!("/api/companies/{b}/records"),
        admin,
        definition,
    )
    .await;
    let path = format!("/api/companies/{b}/records/{definition}");
    let (status, read) = world.call(Method::GET, &path, Some(admin), None).await;
    assert_eq!(status, StatusCode::OK, "{path}: {read}");

    let kind = created(
        &world,
        &format!("/api/companies/{b}/record-types"),
        admin,
        json!({ "allowed_roles": ["admin"], "definition": definition }),
    )
    .await;
    let mut typed = sealed_record_by(&key);
    typed["record_type"] = json!(format!("company:{kind}"));
    created(&world, &format!("/api/companies/{b}/records"), admin, typed).await;

    let file = created(
        &world,
        &format!("/api/companies/{b}/records/{definition}/files"),
        admin,
        json!({ "size": 6 }),
    )
    .await;
    let path = format!("/api/companies/{b}/records/{definition}/files/{file}?offset=0");
    let (status, written) = world.put_octets(&path, admin, b"sealed".to_vec()).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{path}: {written}");
    let (status, bytes) = crate::files::get_part(&world, &format!("{path}&limit=6"), admin).await;
    assert_eq!(status, StatusCode::OK, "{path}");
    assert_eq!(bytes, b"sealed");
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn a_record_read_by_address_is_not_found_through_another_company_its_author_belongs_to() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let both = crate::world::add_member(
        &world,
        &cast.company_b,
        &cast.admin_b,
        "member@alpha.test",
        "member",
    )
    .await;
    assert_eq!(both, cast.member_a_id, "one account in both companies");
    let path = format!(
        "/api/companies/{}/records/{}",
        cast.company_b, cast.record_a
    );
    let (status, body) = world
        .call(Method::GET, &path, Some(&cast.member_a), None)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{path}: {body}");
    let (status, body) = world
        .call(
            Method::PUT,
            &path,
            Some(&cast.member_a),
            Some(crate::records::revision(1, b"through b")),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{path}: {body}");
}
