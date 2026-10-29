//! C2 through the browser's own crypto path: a canary sealed by the panel's crypto module, sent as the browser sends a
//! record, appears neither in the body the server holds, nor in the store's backup, nor in its directory, nor in the
//! company's audit trail; and the store's own passphrase, unsealing the store, opens none of the company vaults.

use axum::http::{Method, StatusCode};
use serde_json::Value;

use tessari_vault_infrastructure::tessaridb::{Answer, Value as StoreValue};

use crate::records::{count, encoded, files_under};
use crate::world::{Cast, World};

/// The create-record body the panel's crypto module makes for `plaintext`, sealed by `node` running it.
fn sealed_by_the_browser_module(company: &str, author_key: &str, plaintext: &str) -> Value {
    let panel = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("panel");
    let output = std::process::Command::new("node")
        .current_dir(&panel)
        .args([
            "tests/support/seal-canary.ts",
            company,
            author_key,
            plaintext,
        ])
        .output()
        .expect("node runs the panel's crypto module");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("a record body")
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN and node"]
async fn a_canary_sealed_by_the_browser_is_absent_from_backup_store_and_audit() {
    let directory = std::env::temp_dir().join(format!("vault-canary-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    let world = World::on_disk(&directory).await;
    let cast = Cast::new(&world).await;
    let canary = "browser-canary-plaintext-7d3a";
    let body = sealed_by_the_browser_module(&cast.company_a, &cast.member_a_key, canary);
    let key_id = body["key_id"].as_str().expect("a key id").to_owned();
    let sent_body = body["body"].clone();
    let (status, created) = world
        .call(
            Method::POST,
            &format!("/api/companies/{}/records", cast.company_a),
            Some(&cast.member_a),
            Some(body),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let record = created["id"].as_str().expect("an id").to_owned();

    // What the server holds once the company's vault is open: anyone on the server side with the company's key sees
    // exactly this, so it is the scan that a browser which skipped sealing would turn red.
    let (status, read) = world
        .call(
            Method::GET,
            &format!("/api/companies/{}/records/{record}", cast.company_a),
            Some(&cast.member_a),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{read}");
    assert_eq!(
        read["body"], sent_body,
        "the server serves the body it was sent"
    );
    let served = tessari_vault_types::base64url::decode(read["body"].as_str().expect("a body"))
        .expect("a base64url body");
    assert_eq!(
        count(&served, canary.as_bytes()),
        0,
        "the canary is in the body the server holds"
    );

    let backup = reqwest::Client::new()
        .get(format!("http://{}/backup", world.node.http))
        .basic_auth(crate::world::STORE_USER, Some(crate::world::STORE_PASSWORD))
        .send()
        .await
        .expect("the backup answers")
        .bytes()
        .await
        .expect("its bytes");
    let (status, trail) = world
        .call(
            Method::GET,
            &format!("/api/companies/{}/audit", cast.company_a),
            Some(&cast.owner_a),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{trail}");
    let trail = trail.to_string().into_bytes();
    let files = files_under(&directory);
    let in_files = |needle: &[u8]| files.iter().map(|file| count(file, needle)).sum::<usize>();

    // The controls: what a record keeps in the clear is found by each scan, so a zero below means absent.
    assert!(
        count(&backup, key_id.as_bytes()) > 0,
        "the scan cannot see the record in the backup"
    );
    assert!(
        in_files(key_id.as_bytes()) > 0,
        "the scan cannot see the record in the store directory"
    );
    assert!(
        count(&trail, record.as_bytes()) > 0,
        "the audit trail does not name the record"
    );
    for needle in [
        canary.as_bytes().to_vec(),
        encoded(canary.as_bytes()).into_bytes(),
    ] {
        assert_eq!(count(&backup, &needle), 0, "the canary is in the backup");
        assert_eq!(in_files(&needle), 0, "the canary is in the store directory");
        assert_eq!(
            count(&trail, &needle),
            0,
            "the canary is in the audit trail"
        );
    }
    drop(world);
    let _ = std::fs::remove_dir_all(&directory);
}

/// The passphrase the store administrator unseals the store with in this test.
const STORE_PASSPHRASE: &str = "the-store-passphrase-of-the-live-suite";

/// Present the store's passphrase on the node's vault surface, for the store or for one vault, as the store's owner.
async fn unseal(world: &World, path: &str) -> (reqwest::StatusCode, String) {
    let answer = reqwest::Client::new()
        .post(format!("http://{}{path}", world.node.http))
        .basic_auth(crate::world::STORE_USER, Some(crate::world::STORE_PASSWORD))
        .body(STORE_PASSPHRASE)
        .send()
        .await
        .expect("the vault surface answers");
    let status = answer.status();
    (status, answer.text().await.expect("its body"))
}

/// Whether the store's owner, with the store unsealed and no key presented, reads `field` of `record` in `vault`.
async fn reveals(world: &World, vault: &str, record: &str, field: &str) -> Result<String, String> {
    match world
        .database
        .run_at(
            &world.place,
            &format!("REVEAL {field} FROM {vault}:'{record}';"),
            Vec::new(),
        )
        .await
    {
        Ok(answers) => Ok(format!("{answers:?}")),
        Err(refused) => Err(refused.to_string()),
    }
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn the_store_passphrase_opens_no_company_vault() {
    let world = World::new().await;
    // The store administrator unseals the store with its own passphrase before any company exists, and it stays open.
    let (status, opened) = unseal(&world, "/vault/unseal").await;
    assert_eq!(status, reqwest::StatusCode::OK, "{opened}");
    assert!(opened.contains("\"unsealed\""), "{opened}");
    let cast = Cast::new(&world).await;
    let companies = [&cast.company_a, &cast.company_b];
    for company in companies {
        let vault = format!("records_{company}");
        world
            .database
            .run_presenting(
                &world.place,
                &format!("CREATE {vault}:'probe' = {{ body: 'company-secret' }};"),
                Vec::new(),
                &vault,
                world
                    .vault_keys
                    .company(company)
                    .expect("the company's key"),
            )
            .await
            .expect("the company's key opens its vault");
    }
    let info = world
        .database
        .run_at(&world.place, "INFO FOR DATABASE;", Vec::new())
        .await
        .expect("the database describes itself");
    let vaults: Vec<String> = match info.last() {
        Some(Answer::Value {
            value: StoreValue::Object(fields),
            ..
        }) => match fields.get("vaults") {
            Some(StoreValue::Array(names)) => names
                .iter()
                .filter_map(|name| match name {
                    StoreValue::String(name) => Some(name.clone()),
                    _ => None,
                })
                .collect(),
            other => panic!("expected the vault names, got {other:?}"),
        },
        other => panic!("expected the database's description, got {other:?}"),
    };
    assert_eq!(
        vaults.len(),
        companies.len(),
        "one vault per company: {vaults:?}"
    );

    // The control: the same act (the store unsealed, no key presented) opens a vault in the store's own custody.
    world
        .database
        .run_at(
            &world.place,
            "DEFINE VAULT control; DEFINE FIELD secret ON control TYPE string SECRET; \
             CREATE control:'probe' = { secret: 'control-secret' };",
            Vec::new(),
        )
        .await
        .expect("a store-custody vault is declared and written while the store is unsealed");
    let control = reveals(&world, "control", "probe", "secret").await;
    assert!(
        control
            .as_ref()
            .is_ok_and(|answer| answer.contains("control-secret")),
        "the unsealed store does not open its own vault: {control:?}"
    );

    let mut opened = 0_usize;
    for vault in &vaults {
        match reveals(&world, vault, "probe", "body").await {
            Ok(answer) => {
                opened = opened.checked_add(1).expect("a handful of vaults");
                eprintln!("{vault} opened: {answer}");
            }
            Err(refused) => assert_eq!(refused, "the vault is sealed", "{vault}"),
        }
    }
    assert_eq!(opened, 0, "company vaults opened by the store's passphrase");
    for vault in &vaults {
        let path = format!(
            "/vault/{}/{}/{vault}/unseal",
            world.place.namespace(),
            world.place.database()
        );
        let (status, refused) = unseal(&world, &path).await;
        assert!(!status.is_success(), "{vault} unsealed: {refused}");
        assert!(
            refused.contains("opens with a key presented with each request"),
            "{vault}: {refused}"
        );
    }
}

/// A snapshot restored into a clean store opens with the company key the service key derives and with nothing else:
/// the backup alone holds no key, the key file alone holds no data. The restore is timed.
#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn a_snapshot_restored_into_a_clean_store_opens_with_the_service_key_alone() {
    use std::time::Instant;
    use tessari_vault_infrastructure::tessaridb::{Database, StoreConfig};

    let root = std::env::temp_dir().join(format!("vault-restore-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (source, restored) = (root.join("source"), root.join("restored"));
    let world = World::on_disk(&source).await;
    let cast = Cast::new(&world).await;
    let vault = crate::records::vault(&cast.company_a);
    let key_a = world.vault_keys.company(&cast.company_a).expect("KV(A)");
    world
        .database
        .run_presenting(
            &world.place,
            &format!("CREATE {vault}:'restore-probe' = {{ body: 'restored-canary' }};"),
            Vec::new(),
            &vault,
            key_a,
        )
        .await
        .expect("company A's key writes its vault");
    let snapshot = reqwest::Client::new()
        .get(format!("http://{}/backup", world.node.http))
        .basic_auth(crate::world::STORE_USER, Some(crate::world::STORE_PASSWORD))
        .send()
        .await
        .expect("the backup answers")
        .bytes()
        .await
        .expect("its bytes");
    let artifact = root.join("vault.tessarisnap");
    std::fs::write(&artifact, &snapshot).expect("the artifact is written");

    let started = Instant::now();
    let ran = std::process::Command::new(std::env::var("TESSARIDB_BIN").expect("TESSARIDB_BIN"))
        .arg(&restored)
        .arg("--restore")
        .arg(&artifact)
        .output()
        .expect("the engine runs");
    let took = started.elapsed();
    assert!(
        ran.status.success(),
        "{}",
        String::from_utf8_lossy(&ran.stderr)
    );
    eprintln!(
        "restore: {} bytes in {} ms",
        snapshot.len(),
        took.as_millis()
    );

    let node = crate::world::Node::start_in(Some(&restored)).await;
    let database = Database::new(
        StoreConfig {
            address: node.wire.clone(),
            user: crate::world::STORE_USER.to_owned(),
            password: crate::world::STORE_PASSWORD.to_owned(),
            authority: None,
        },
        2,
    )
    .expect("a loopback pool");
    let reveal = format!("REVEAL body FROM {vault}:'restore-probe';");
    let opened = database
        .run_presenting(
            &world.place,
            &reveal,
            Vec::new(),
            &vault,
            world
                .vault_keys
                .company(&cast.company_a)
                .expect("KV(A) again"),
        )
        .await
        .expect("the restored vault opens with the derived key");
    let Some(Answer::Value {
        value: StoreValue::Object(fields),
        ..
    }) = opened.last()
    else {
        panic!("expected the revealed body, got {opened:?}");
    };
    assert_eq!(
        fields.get("body"),
        Some(&StoreValue::from("restored-canary")),
        "{fields:?}"
    );
    let key_b = world.vault_keys.company(&cast.company_b).expect("KV(B)");
    let refused = database
        .run_presenting(&world.place, &reveal, Vec::new(), &vault, key_b)
        .await
        .expect_err("another company's key opened the restored vault");
    assert!(
        refused.to_string().contains("the key is wrong"),
        "{refused:?}"
    );
    assert!(
        database
            .run_at(&world.place, &reveal, Vec::new())
            .await
            .is_err(),
        "the restored vault opened with no key presented"
    );
    drop((node, world));
    let _ = std::fs::remove_dir_all(&root);
}
