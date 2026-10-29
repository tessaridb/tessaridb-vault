//! C2 through the browser's own crypto path: a canary sealed by the panel's crypto module, sent as the browser sends a
//! record, appears neither in the body the server holds, nor in the store's backup, nor in its directory, nor in the
//! company's audit trail.

use axum::http::{Method, StatusCode};
use serde_json::Value;

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
