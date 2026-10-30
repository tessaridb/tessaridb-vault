//! Encrypted files on a record: a small one kept in the company's records vault, a larger one in a bucket, each
//! written in parts only in order, read only when whole, and reached only by the record's author.

use async_trait::async_trait;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use serde_json::{Value, json};
use tessari_vault_infrastructure::s3::ObjectStorage;
use tessari_vault_infrastructure::tessaridb::Value as StoreValue;
use tessari_vault_infrastructure::{Error, Result};
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

#[derive(Default)]
pub(crate) struct FakeObjects {
    values: std::sync::Mutex<std::collections::BTreeMap<String, Vec<u8>>>,
    fail_put: bool,
    fail_delete: bool,
}

impl FakeObjects {
    pub(crate) fn keys(&self) -> Vec<String> {
        self.values
            .lock()
            .expect("object lock")
            .keys()
            .cloned()
            .collect()
    }
}

fn unavailable() -> Error {
    Error::Unavailable {
        message: "the object double is unavailable".to_owned(),
    }
}

#[async_trait]
impl ObjectStorage for FakeObjects {
    async fn put(&self, key: &str, bytes: &[u8]) -> Result<()> {
        if self.fail_put {
            return Err(unavailable());
        }
        self.values
            .lock()
            .expect("object lock")
            .insert(key.to_owned(), bytes.to_vec());
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Vec<u8>> {
        self.values
            .lock()
            .expect("object lock")
            .get(key)
            .cloned()
            .ok_or_else(unavailable)
    }

    async fn delete(&self, key: &str) -> Result<()> {
        if self.fail_delete {
            return Err(unavailable());
        }
        self.values.lock().expect("object lock").remove(key);
        Ok(())
    }
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn a_large_file_on_s3_round_trips_and_is_deleted_with_its_metadata() {
    let objects = std::sync::Arc::new(FakeObjects::default());
    let world = World::with_objects(objects.clone()).await;
    let cast = Cast::new(&world).await;
    let first = noise(4_194_304, 17);
    let second = noise(1_000_000, 23);
    let (path, created) = new_file(&world, &cast, first.len() + second.len()).await;
    assert_eq!(created["kept"], "s3");
    let (status, body) = world
        .put_octets(&format!("{path}?offset=0"), &cast.member_a, first.clone())
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let (status, body) = world
        .put_octets(
            &format!("{path}?offset={}", first.len()),
            &cast.member_a,
            second.clone(),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    assert_eq!(objects.keys().len(), 2);
    let (status, read) = get_part(
        &world,
        &format!("{path}?offset={}&limit={}", first.len() - 5, 10),
        &cast.member_a,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&read));
    assert_eq!(read[..5], first[first.len() - 5..]);
    assert_eq!(read[5..], second[..5]);
    let (status, deleted) = world
        .call(Method::DELETE, &path, Some(&cast.member_a), None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{deleted}");
    assert!(objects.keys().is_empty());
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn an_s3_failure_publishes_no_part_and_a_finalization_conflict_leaves_it_unreadable() {
    let objects = std::sync::Arc::new(FakeObjects {
        fail_put: true,
        ..FakeObjects::default()
    });
    let world = World::with_objects(objects.clone()).await;
    let cast = Cast::new(&world).await;
    let bytes = noise(4_194_304, 29);
    let (path, _) = new_file(&world, &cast, bytes.len()).await;
    let (status, body) = world
        .put_octets(&format!("{path}?offset=0"), &cast.member_a, bytes.clone())
        .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    let (status, _) = get_part(&world, &format!("{path}?offset=0&limit=10"), &cast.member_a).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(objects.keys().is_empty());

    let objects = std::sync::Arc::new(FakeObjects::default());
    let world = World::with_objects(objects.clone()).await;
    let cast = Cast::new(&world).await;
    let (path, created) = new_file(&world, &cast, bytes.len()).await;
    let file = created["id"].as_str().expect("an id");
    world
        .database
        .run_at(
            &world.place,
            "CREATE file_objects = { company: $company, file: $file, offset: 0, object_key: 'other', size: 1 };",
            vec![
                ("company".to_owned(), StoreValue::from(cast.company_a.as_str())),
                ("file".to_owned(), StoreValue::from(file)),
            ],
        )
        .await
        .expect("a conflicting published part");
    let (status, _) = world
        .put_octets(&format!("{path}?offset=0"), &cast.member_a, bytes.clone())
        .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        objects.keys().len(),
        1,
        "the uploaded attempt is orphaned, not published"
    );
    let (status, _) = get_part(&world, &format!("{path}?offset=0&limit=10"), &cast.member_a).await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN and a live S3 node"]
async fn a_live_s3_node_round_trips_ciphertext_and_refuses_a_wrong_secret() {
    use tessari_vault_infrastructure::config::S3Config;
    use tessari_vault_infrastructure::s3::S3;

    let config = S3Config {
        endpoint: std::env::var("TESSARIDB_VAULT_TEST_S3_ENDPOINT").expect("S3 endpoint"),
        bucket: std::env::var("TESSARIDB_VAULT_TEST_S3_BUCKET").expect("S3 bucket"),
        access_key: std::env::var("TESSARIDB_VAULT_TEST_S3_ACCESS_KEY").expect("S3 access key"),
        secret_key: std::env::var("TESSARIDB_VAULT_TEST_S3_SECRET_KEY").expect("S3 secret"),
        region: std::env::var("TESSARIDB_VAULT_TEST_S3_REGION")
            .unwrap_or_else(|_| "us-east-1".to_owned()),
    };
    let world =
        World::with_objects(std::sync::Arc::new(S3::new(&config).expect("S3 client"))).await;
    let cast = Cast::new(&world).await;
    let bytes = noise(4_194_304, 31);
    let (path, created) = new_file(&world, &cast, bytes.len()).await;
    assert_eq!(created["kept"], "s3");
    let (status, body) = world
        .put_octets(&format!("{path}?offset=0"), &cast.member_a, bytes.clone())
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let (status, read) = get_part(
        &world,
        &format!("{path}?offset=100&limit=1000"),
        &cast.member_a,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(read, bytes[100..1100]);
    let (status, deleted) = world
        .call(Method::DELETE, &path, Some(&cast.member_a), None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{deleted}");
    let client = tessaridb_s3_client::ClientBuilder::new(
        &config.endpoint,
        tessaridb_s3_client::sign::Credentials::new(&config.access_key, &config.secret_key),
    )
    .region(&config.region)
    .build()
    .expect("listing client");
    let bucket = tessaridb_s3_client::BucketName::new(&config.bucket).expect("bucket");
    let remaining = client
        .list_objects(&bucket, &tessaridb_s3_client::ListObjects::default())
        .await
        .expect("the listing");
    assert!(
        remaining.objects.is_empty(),
        "the file's S3 object is deleted"
    );

    let mut wrong = config;
    wrong.secret_key = "definitely-not-the-application-secret".to_owned();
    let objects = S3::new(&wrong).expect("wrong-secret client");
    let key = format!(
        "{}/{}/0/wrong-secret",
        cast.company_a,
        created["id"].as_str().expect("file")
    );
    let error = objects
        .put(&key, b"not accepted")
        .await
        .expect_err("wrong secret refused");
    assert!(matches!(error, Error::Unavailable { .. }), "{error}");
}
