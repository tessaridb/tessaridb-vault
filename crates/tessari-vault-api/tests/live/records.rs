//! Records: each company's records vault is in key custody and opens only with that company's key.

use axum::http::{Method, StatusCode};
use tessari_vault_infrastructure::tessaridb::{Answer, Value};

use crate::world::{Cast, World};

fn vault(company: &str) -> String {
    format!("records_{company}")
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn each_company_gets_a_records_vault_that_only_its_own_key_opens() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let vault_a = vault(&cast.company_a);
    let info = world
        .database
        .run_at(
            &world.place,
            &format!("INFO FOR VAULT {vault_a};"),
            Vec::new(),
        )
        .await
        .expect("the company's vault exists");
    let Some(Answer::Value {
        value: Value::Object(fields),
        ..
    }) = info.last()
    else {
        panic!("expected the vault's description, got {info:?}");
    };
    assert_eq!(
        fields.get("custody"),
        Some(&Value::from("key")),
        "{fields:?}"
    );

    let key_a = world.vault_keys.company(&cast.company_a).expect("KV(A)");
    let key_b = world.vault_keys.company(&cast.company_b).expect("KV(B)");
    world
        .database
        .run_presenting(
            &world.place,
            &format!("CREATE {vault_a}:'probe' = {{ body: 'sealed' }};"),
            Vec::new(),
            &vault_a,
            key_a,
        )
        .await
        .expect("company A's key opens company A's vault");
    let refused = world
        .database
        .run_presenting(
            &world.place,
            &format!("REVEAL body FROM {vault_a}:'probe';"),
            Vec::new(),
            &vault_a,
            key_b,
        )
        .await
        .expect_err("company B's key opened company A's vault");
    assert!(
        refused.to_string().contains("the key is wrong"),
        "refused otherwise: {refused:?}"
    );
}

pub(crate) fn encoded(bytes: &[u8]) -> String {
    tessari_vault_types::base64url::encode(bytes)
}

/// A new record whose sealed body is `body`, as the browser sends one.
fn new_record(body: &[u8]) -> serde_json::Value {
    serde_json::json!({
        "record_type": "login",
        "key_id": "11".repeat(16),
        "body": encoded(body),
        "signature": encoded(&[7; 64]),
        "author_key": "22".repeat(16),
        "wrap": encoded(&[9; 82]),
    })
}

fn revision(expected: i64, body: &[u8]) -> serde_json::Value {
    serde_json::json!({
        "revision": expected,
        "key_id": "33".repeat(16),
        "body": encoded(body),
        "signature": encoded(&[8; 64]),
        "author_key": "22".repeat(16),
        "wrap": encoded(&[10; 82]),
    })
}

async fn create(world: &World, company: &str, token: &str, body: &[u8]) -> String {
    let (status, answer) = world
        .call(
            Method::POST,
            &format!("/api/companies/{company}/records"),
            Some(token),
            Some(new_record(body)),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{answer}");
    answer["id"].as_str().expect("an id").to_owned()
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn a_record_round_trips_by_bytes_and_lists_only_for_its_author() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let a = &cast.company_a;
    let id = create(&world, a, &cast.member_a, b"sealed body bytes").await;
    let (status, read) = world
        .call(
            Method::GET,
            &format!("/api/companies/{a}/records/{id}"),
            Some(&cast.member_a),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{read}");
    assert_eq!(read["body"], encoded(b"sealed body bytes"));
    assert_eq!(read["signature"], encoded(&[7; 64]));
    assert_eq!(read["wrap"], encoded(&[9; 82]));
    assert_eq!(read["author_key"], "22".repeat(16));
    assert_eq!(read["record"]["key_id"], "11".repeat(16));
    assert_eq!(read["record"]["revision"], 1);
    assert_eq!(read["record"]["record_type"], "login");
    assert_eq!(read["record"]["author"], cast.member_a_id.as_str());

    let listed = |token: String| {
        let world = &world;
        async move {
            let (status, list) = world
                .call(
                    Method::GET,
                    &format!("/api/companies/{a}/records"),
                    Some(&token),
                    None,
                )
                .await;
            assert_eq!(status, StatusCode::OK, "{list}");
            list["records"]
                .as_array()
                .expect("a list")
                .iter()
                .map(|record| record["id"].as_str().expect("an id").to_owned())
                .collect::<Vec<_>>()
        }
    };
    let mut expected = vec![cast.record_a.clone(), id.clone()];
    expected.sort();
    let mut mine = listed(cast.member_a.clone()).await;
    mine.sort();
    assert_eq!(mine, expected);
    assert!(
        listed(cast.owner_a.clone()).await.is_empty(),
        "another member's record listed"
    );
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn a_record_is_not_found_for_anyone_but_its_author() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let a = &cast.company_a;
    let id = create(&world, a, &cast.member_a, b"mine").await;
    for (who, path) in [
        (&cast.owner_a, format!("/api/companies/{a}/records/{id}")),
        (&cast.admin_b, format!("/api/companies/{a}/records/{id}")),
        (
            &cast.admin_b,
            format!("/api/companies/{}/records/{id}", cast.company_b),
        ),
    ] {
        let (status, body) = world.call(Method::GET, &path, Some(who), None).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}: {body}");
        let (status, body) = world
            .call(Method::PUT, &path, Some(who), Some(revision(1, b"theirs")))
            .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}: {body}");
    }
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn a_revision_is_written_only_over_the_revision_it_read() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let a = &cast.company_a;
    let id = create(&world, a, &cast.member_a, b"first").await;
    let path = format!("/api/companies/{a}/records/{id}");
    let (status, revised) = world
        .call(
            Method::PUT,
            &path,
            Some(&cast.member_a),
            Some(revision(1, b"second")),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{revised}");
    assert_eq!(revised["revision"], 2);
    let (_, read) = world
        .call(Method::GET, &path, Some(&cast.member_a), None)
        .await;
    assert_eq!(read["body"], encoded(b"second"));
    assert_eq!(read["record"]["key_id"], "33".repeat(16));

    let (status, stale) = world
        .call(
            Method::PUT,
            &path,
            Some(&cast.member_a),
            Some(revision(1, b"late")),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{stale}");
    assert_eq!(stale["error_code"], "stale_revision");

    let racing = |body: &'static [u8]| {
        world.call(
            Method::PUT,
            &path,
            Some(&cast.member_a),
            Some(revision(2, body)),
        )
    };
    let (one, two) = tokio::join!(racing(b"third-a"), racing(b"third-b"));
    let mut statuses = [one.0, two.0];
    statuses.sort();
    assert_eq!(
        statuses,
        [StatusCode::OK, StatusCode::CONFLICT],
        "{one:?} {two:?}"
    );
    let (_, read) = world
        .call(Method::GET, &path, Some(&cast.member_a), None)
        .await;
    assert_eq!(read["record"]["revision"], 3);
}

/// How many times `needle` appears in `haystack`.
pub(crate) fn count(haystack: &[u8], needle: &[u8]) -> usize {
    haystack
        .windows(needle.len())
        .filter(|window| *window == needle)
        .count()
}

pub(crate) fn files_under(directory: &std::path::Path) -> Vec<Vec<u8>> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(directory).expect("the store directory") {
        let path = entry.expect("an entry").path();
        if path.is_dir() {
            found.extend(files_under(&path));
        } else {
            found.push(std::fs::read(&path).expect("a store file"));
        }
    }
    found
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn a_record_body_is_absent_from_the_backup_and_the_store_directory() {
    let directory = std::env::temp_dir().join(format!("vault-raw-scan-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    let world = World::on_disk(&directory).await;
    let cast = Cast::new(&world).await;
    let sealed = b"raw-scan-canary-sealed-4b1f".as_slice();
    let plain = b"raw-scan-canary-plain-9c2e".as_slice();
    create(&world, &cast.company_a, &cast.member_a, sealed).await;
    // The control: the same kind of value in an ordinary record, which the scan must find.
    world
        .database
        .run_at(
            &world.place,
            "DEFINE COLLECTION IF NOT EXISTS raw_scan_control; CREATE raw_scan_control = { note: $note };",
            vec![(
                "note".to_owned(),
                Value::from(std::str::from_utf8(plain).expect("text")),
            )],
        )
        .await
        .expect("the control record");

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
    let encoded_sealed = encoded(sealed);
    assert!(
        count(&backup, plain) > 0,
        "the scan cannot see an ordinary value in the backup"
    );
    assert!(
        in_files(plain) > 0,
        "the scan cannot see an ordinary value in the store directory"
    );
    assert_eq!(
        count(&backup, sealed),
        0,
        "the sealed body is in the backup"
    );
    assert_eq!(
        count(&backup, encoded_sealed.as_bytes()),
        0,
        "the sealed body is in the backup"
    );
    assert_eq!(
        in_files(sealed),
        0,
        "the sealed body is in the store directory"
    );
    assert_eq!(
        in_files(encoded_sealed.as_bytes()),
        0,
        "the sealed body is in the store directory"
    );
    drop(world);
    let _ = std::fs::remove_dir_all(&directory);
}
