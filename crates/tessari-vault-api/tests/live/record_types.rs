//! Record types a company defines: admins define them over a sealed definition record of their own, every member
//! sees them, and only the roles a type admits create records of it.

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use crate::world::{Cast, World, sealed_record_by};

/// A record of `record_type` as the browser of the member who registered `key` sends one.
fn record_of(record_type: &str, key: &str) -> Value {
    let mut body = sealed_record_by(key);
    body["record_type"] = json!(record_type);
    body
}

async fn post(world: &World, path: &str, token: &str, body: Value) -> (StatusCode, Value) {
    world
        .call(Method::POST, path, Some(token), Some(body))
        .await
}

/// An admin's definition record and a type over it admitting `allowed`, answering the type's record-type id.
async fn defined(world: &World, cast: &Cast, allowed: &[&str]) -> String {
    let a = &cast.company_a;
    let (status, definition) = post(
        world,
        &format!("/api/companies/{a}/records"),
        &cast.admin_a,
        record_of("record-type", &cast.admin_a_key),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{definition}");
    let (status, defined) = post(
        world,
        &format!("/api/companies/{a}/record-types"),
        &cast.admin_a,
        json!({ "allowed_roles": allowed, "definition": definition["id"] }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{defined}");
    format!("company:{}", defined["id"].as_str().expect("an id"))
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn only_the_roles_a_company_type_admits_create_records_of_it() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let a = &cast.company_a;
    let records = format!("/api/companies/{a}/records");
    let kind = defined(&world, &cast, &["admin"]).await;

    let (status, refused) = post(
        &world,
        &records,
        &cast.member_a,
        record_of(&kind, &cast.member_a_key),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{refused}");
    for (token, key) in [
        (&cast.admin_a, &cast.admin_a_key),
        (&cast.owner_a, &cast.owner_a_key),
    ] {
        let (status, created) = post(&world, &records, token, record_of(&kind, key)).await;
        assert_eq!(status, StatusCode::CREATED, "{created}");
        assert_eq!(created["record_type"], kind.as_str());
    }
    let open = defined(&world, &cast, &["member"]).await;
    let (status, created) = post(
        &world,
        &records,
        &cast.member_a,
        record_of(&open, &cast.member_a_key),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn members_see_the_types_and_neither_define_them_nor_keep_definitions() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let a = &cast.company_a;
    let kind = defined(&world, &cast, &["admin"]).await;
    let (status, listed) = world
        .call(
            Method::GET,
            &format!("/api/companies/{a}/record-types"),
            Some(&cast.member_a),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{listed}");
    let types = listed["record_types"].as_array().expect("a list");
    assert_eq!(types.len(), 1, "{listed}");
    assert_eq!(
        format!("company:{}", types[0]["id"].as_str().expect("an id")),
        kind
    );
    assert_eq!(types[0]["allowed_roles"], json!(["admin"]));

    let (status, refused) = post(
        &world,
        &format!("/api/companies/{a}/records"),
        &cast.member_a,
        record_of("record-type", &cast.member_a_key),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{refused}");
    let (status, refused) = post(
        &world,
        &format!("/api/companies/{a}/record-types"),
        &cast.member_a,
        json!({ "allowed_roles": ["member"], "definition": cast.record_a }),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{refused}");
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn a_type_rests_on_its_definers_own_definition_and_stays_in_its_company() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let a = &cast.company_a;
    // Member A's login record is neither the admin's nor a definition.
    let (status, refused) = post(
        &world,
        &format!("/api/companies/{a}/record-types"),
        &cast.admin_a,
        json!({ "allowed_roles": ["member"], "definition": cast.record_a }),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{refused}");
    let kind = defined(&world, &cast, &["member"]).await;
    let (status, refused) = post(
        &world,
        &format!("/api/companies/{}/records", cast.company_b),
        &cast.admin_b,
        record_of(&kind, &cast.member_a_key),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{refused}");
}
