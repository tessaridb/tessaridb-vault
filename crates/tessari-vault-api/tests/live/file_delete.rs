//! A file its uploader no longer wants, such as one a save that failed never named, is deleted by that uploader alone:
//! another editor of the record, another member and another company find no such file, and the deletion is on the
//! company's trail.

use axum::http::{Method, StatusCode};

use crate::files::{get_part, new_file};
use crate::shares::{shares, to_member};
use crate::world::{Cast, World};

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn only_its_uploader_deletes_a_file() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    // Owner A may edit member A's record, so the record reaches them; the file is still not theirs.
    let (status, body) = world
        .call(
            Method::POST,
            &shares(&cast, &cast.record_a),
            Some(&cast.member_a),
            Some(to_member(&cast.owner_a_id, &cast.owner_a_key, "edit", 8)),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let (path, _) = new_file(&world, &cast, 6).await;
    let (status, body) = world
        .put_octets(
            &format!("{path}?offset=0"),
            &cast.member_a,
            b"sealed".to_vec(),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let other_company = path.replace(&cast.company_a, &cast.company_b);
    for (who, at) in [
        (&cast.owner_a, path.as_str()),
        (&cast.admin_a, path.as_str()),
        (&cast.admin_b, other_company.as_str()),
    ] {
        let (status, body) = world.call(Method::DELETE, at, Some(who), None).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{at}: {body}");
    }
    let (status, _) = get_part(&world, &format!("{path}?offset=0&limit=6"), &cast.member_a).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "the file went with a refused deletion"
    );

    let (status, body) = world
        .call(Method::DELETE, &path, Some(&cast.member_a), None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let (status, _) = get_part(&world, &format!("{path}?offset=0&limit=6"), &cast.member_a).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "a deleted file still reads");
    let (status, _) = world
        .call(Method::DELETE, &path, Some(&cast.member_a), None)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "deleted twice");
    let (status, trail) = world
        .call(
            Method::GET,
            &format!("/api/companies/{}/audit", cast.company_a),
            Some(&cast.owner_a),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{trail}");
    let deleted = trail["events"]
        .as_array()
        .expect("events")
        .iter()
        .filter(|event| event["action"] == "file.delete")
        .count();
    assert_eq!(deleted, 1, "{trail}");
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn a_bucket_file_cut_short_is_deleted_with_its_parts() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let (path, created) = new_file(&world, &cast, 4_194_304 + 10).await;
    assert_eq!(created["kept"], "bucket");
    let (status, body) = world
        .put_octets(
            &format!("{path}?offset=0"),
            &cast.member_a,
            vec![7; 4_194_304],
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");

    let (status, body) = world
        .call(Method::DELETE, &path, Some(&cast.member_a), None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let (status, _) = get_part(&world, &format!("{path}?offset=0&limit=10"), &cast.member_a).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "a deleted file still reads");
}
