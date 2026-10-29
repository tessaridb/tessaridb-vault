//! Encrypted files on a record: a small one kept in the company's records vault, a larger one in a bucket, each
//! written in parts only in order, read only when whole, and reached only by the record's author.

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::world::{Cast, World};

/// A file's bytes from `offset`, at most `limit` of them.
pub(crate) async fn get_part(world: &World, path: &str, token: &str) -> (StatusCode, Vec<u8>) {
    let request = Request::builder()
        .method(Method::GET)
        .uri(path)
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .expect("a request");
    let response = world.app.clone().oneshot(request).await.expect("an answer");
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 8 << 20)
        .await
        .expect("a body");
    (status, bytes.to_vec())
}

/// Bytes that differ along their length, so a part written at the wrong place does not read back the same.
fn noise(length: usize, seed: u8) -> Vec<u8> {
    (0..length)
        .map(|index| u8::try_from(index % 251).expect("small").wrapping_add(seed))
        .collect()
}

pub(crate) async fn new_file(world: &World, cast: &Cast, size: usize) -> (String, Value) {
    let (status, created) = world
        .call(
            Method::POST,
            &format!(
                "/api/companies/{}/records/{}/files",
                cast.company_a, cast.record_a
            ),
            Some(&cast.member_a),
            Some(json!({ "size": size })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let id = created["id"].as_str().expect("an id").to_owned();
    (
        format!(
            "/api/companies/{}/records/{}/files/{id}",
            cast.company_a, cast.record_a
        ),
        created,
    )
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn a_small_file_is_kept_in_the_vault_and_reads_back_whole() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let bytes = noise(1_000, 3);
    let (path, created) = new_file(&world, &cast, bytes.len()).await;
    assert_eq!(created["kept"], "vault");
    let (status, early) = get_part(
        &world,
        &format!("{path}?offset=0&limit=1000"),
        &cast.member_a,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "{}",
        String::from_utf8_lossy(&early)
    );
    let (status, body) = world
        .put_octets(&format!("{path}?offset=0"), &cast.member_a, bytes.clone())
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let (status, read) = get_part(
        &world,
        &format!("{path}?offset=0&limit=1000"),
        &cast.member_a,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(read, bytes);
    let (status, part) = get_part(
        &world,
        &format!("{path}?offset=100&limit=10"),
        &cast.member_a,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(part, bytes[100..110]);
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn a_large_file_is_written_in_order_in_parts_and_read_by_ranges() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let first = noise(4_194_304, 5);
    let second = noise(1_000_000, 9);
    let (path, created) = new_file(&world, &cast, first.len() + second.len()).await;
    assert_eq!(created["kept"], "bucket");
    let at = |offset: usize| format!("{path}?offset={offset}");

    let (status, body) = world
        .put_octets(&at(first.len()), &cast.member_a, second.clone())
        .await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "a part ahead of the file was taken: {body}"
    );
    assert_eq!(body["error_code"], "out_of_order");
    let (status, body) = world
        .put_octets(&at(0), &cast.member_a, first[..1_000].to_vec())
        .await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "a short part that does not end the file was taken: {body}"
    );
    let (status, body) = world
        .put_octets(&at(0), &cast.member_a, first.clone())
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let (status, early) =
        get_part(&world, &format!("{path}?offset=0&limit=10"), &cast.member_a).await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "{}",
        String::from_utf8_lossy(&early)
    );
    let (status, body) = world
        .put_octets(&at(0), &cast.member_a, first.clone())
        .await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "a part written twice was taken: {body}"
    );
    let mut past = second.clone();
    past.push(0);
    let (status, body) = world
        .put_octets(&at(first.len()), &cast.member_a, past)
        .await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "a part past the size was taken: {body}"
    );
    let (status, body) = world
        .put_octets(&at(first.len()), &cast.member_a, second.clone())
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");

    let (status, head) = get_part(
        &world,
        &format!("{path}?offset=0&limit=4194304"),
        &cast.member_a,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(head, first);
    let tail_at = first.len() - 5;
    let (status, seam) = get_part(
        &world,
        &format!("{path}?offset={tail_at}&limit=10"),
        &cast.member_a,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(seam[..5], first[tail_at..]);
    assert_eq!(seam[5..], second[..5]);
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn a_file_is_not_found_for_anyone_but_the_records_author() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let bytes = noise(64, 1);
    let (path, _) = new_file(&world, &cast, bytes.len()).await;
    let (status, _) = world
        .put_octets(&format!("{path}?offset=0"), &cast.member_a, bytes.clone())
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    for who in [&cast.owner_a, &cast.admin_b] {
        let (status, _) = get_part(&world, &format!("{path}?offset=0&limit=64"), who).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let (status, _) = world
            .put_octets(&format!("{path}?offset=0"), who, bytes.clone())
            .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let (status, _) = world
            .call(
                Method::POST,
                &format!(
                    "/api/companies/{}/records/{}/files",
                    cast.company_a, cast.record_a
                ),
                Some(who),
                Some(json!({ "size": 10 })),
            )
            .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn a_small_file_is_absent_from_the_backup_and_the_store_directory() {
    use crate::records::{count, encoded, files_under};
    let directory = std::env::temp_dir().join(format!("vault-file-scan-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    let world = World::on_disk(&directory).await;
    let cast = Cast::new(&world).await;
    let sealed = b"file-scan-canary-sealed-7d3a".to_vec();
    let (path, created) = new_file(&world, &cast, sealed.len()).await;
    let (status, body) = world
        .put_octets(&format!("{path}?offset=0"), &cast.member_a, sealed.clone())
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");

    let backup = reqwest::Client::new()
        .get(format!("http://{}/backup", world.node.http))
        .basic_auth(crate::world::STORE_USER, Some(crate::world::STORE_PASSWORD))
        .send()
        .await
        .expect("the backup answers")
        .bytes()
        .await
        .expect("its bytes");
    let files = files_under(&directory);
    let in_files = |needle: &[u8]| files.iter().map(|file| count(file, needle)).sum::<usize>();
    // The control is the file's own id, an ordinary value on the `files` row, which the scan must find.
    let id = created["id"].as_str().expect("an id").as_bytes().to_vec();
    assert!(
        count(&backup, &id) > 0,
        "the scan cannot see the file's row in the backup"
    );
    assert!(
        in_files(&id) > 0,
        "the scan cannot see the file's row in the store directory"
    );
    for needle in [sealed.clone(), encoded(&sealed).into_bytes()] {
        assert_eq!(
            count(&backup, &needle),
            0,
            "the sealed file is in the backup"
        );
        assert_eq!(
            in_files(&needle),
            0,
            "the sealed file is in the store directory"
        );
    }
    drop(world);
    let _ = std::fs::remove_dir_all(&directory);
}
