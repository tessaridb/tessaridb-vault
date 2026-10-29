//! A snapshot restored into a clean store: it opens with the company key the service key derives and with nothing
//! else, and a copy taken before a company was deleted stays closed once the epoch that company was keyed under is
//! retired from the key ring.

use std::path::{Path, PathBuf};
use std::time::Instant;

use tessari_vault_infrastructure::custody::{KeyRing, VaultKeys};
use tessari_vault_infrastructure::tessaridb::{
    Answer, Database, StoreConfig, Value as StoreValue, VaultKey,
};

use crate::world::{Cast, Node, World};

/// A world on disk under a fresh `root`, company A's vault holding one canary record under the current epoch.
async fn with_canary(name: &str) -> (PathBuf, World, Cast, String) {
    let root = std::env::temp_dir().join(format!("vault-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let world = World::on_disk(&root.join("source")).await;
    let cast = Cast::new(&world).await;
    let vault = crate::records::vault(&cast.company_a);
    let key = world
        .vault_keys
        .company(world.vault_keys.current_epoch(), &cast.company_a)
        .expect("KV(A)");
    world
        .database
        .run_presenting(
            &world.place,
            &format!("CREATE {vault}:'restore-probe' = {{ body: 'restored-canary' }};"),
            Vec::new(),
            &vault,
            key,
        )
        .await
        .expect("company A's key writes its vault");
    (root, world, cast, vault)
}

/// The world's store backed up whole and restored into a clean store under `root`, served by a node of its own.
async fn restored(world: &World, root: &Path) -> (Node, Database) {
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
    let into = root.join("restored");
    let started = Instant::now();
    let ran = std::process::Command::new(std::env::var("TESSARIDB_BIN").expect("TESSARIDB_BIN"))
        .arg(&into)
        .arg("--restore")
        .arg(&artifact)
        .output()
        .expect("the engine runs");
    assert!(
        ran.status.success(),
        "{}",
        String::from_utf8_lossy(&ran.stderr)
    );
    eprintln!(
        "restore: {} bytes in {} ms",
        snapshot.len(),
        started.elapsed().as_millis()
    );
    let node = Node::start_in(Some(&into)).await;
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
    (node, database)
}

/// Reveal the canary from `vault` with `key`: its body, or the store's refusal in words.
async fn canary(
    database: &Database,
    world: &World,
    vault: &str,
    key: VaultKey,
) -> Result<StoreValue, String> {
    let opened = database
        .run_presenting(
            &world.place,
            &format!("REVEAL body FROM {vault}:'restore-probe';"),
            Vec::new(),
            vault,
            key,
        )
        .await
        .map_err(|refused| refused.to_string())?;
    let Some(Answer::Value {
        value: StoreValue::Object(fields),
        ..
    }) = opened.last()
    else {
        panic!("expected the revealed body, got {opened:?}");
    };
    Ok(fields.get("body").cloned().expect("a body"))
}

/// The restore is timed.
#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn a_snapshot_restored_into_a_clean_store_opens_with_the_service_key_alone() {
    let (root, world, cast, vault) = with_canary("restore").await;
    let (node, database) = restored(&world, &root).await;
    let epoch = world.vault_keys.current_epoch();
    let own = world
        .vault_keys
        .company(epoch, &cast.company_a)
        .expect("KV(A)");
    assert_eq!(
        canary(&database, &world, &vault, own).await,
        Ok(StoreValue::from("restored-canary"))
    );
    let other = world
        .vault_keys
        .company(epoch, &cast.company_b)
        .expect("KV(B)");
    let refused = canary(&database, &world, &vault, other)
        .await
        .expect_err("another company's key opened the restored vault");
    assert!(refused.contains("the key is wrong"), "{refused}");
    assert!(
        database
            .run_at(
                &world.place,
                &format!("REVEAL body FROM {vault}:'restore-probe';"),
                Vec::new()
            )
            .await
            .is_err(),
        "the restored vault opened with no key presented"
    );
    drop((node, world));
    let _ = std::fs::remove_dir_all(&root);
}

/// After a rotation the ring holds a new epoch; once the old one is retired, nothing the operator still holds opens
/// the copy, which is how a deletion reaches backups older than it.
#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn a_copy_from_before_a_deletion_stays_closed_once_its_epoch_is_retired() {
    let (root, world, cast, vault) = with_canary("retired").await;
    let (node, database) = restored(&world, &root).await;
    let kept_under = world.vault_keys.current_epoch();
    // Control: the copy is real, and opens while its epoch is still in the ring.
    let still_held = world
        .vault_keys
        .company(kept_under, &cast.company_a)
        .expect("KV(A)");
    assert_eq!(
        canary(&database, &world, &vault, still_held).await,
        Ok(StoreValue::from("restored-canary"))
    );

    let next = kept_under.checked_add(1).expect("a next epoch");
    let key_file = root.join("service-key-retired");
    std::fs::write(&key_file, format!("{next} {}\n", "5a".repeat(32))).expect("the key file");
    std::fs::set_permissions(
        &key_file,
        std::os::unix::fs::PermissionsExt::from_mode(0o600),
    )
    .expect("its mode");
    let retired = VaultKeys::new(&KeyRing::load(&key_file).expect("the ring")).expect("its keys");
    assert!(
        retired.company(kept_under, &cast.company_a).is_err(),
        "a retired epoch still derives the company's key"
    );
    let current = retired
        .company(next, &cast.company_a)
        .expect("KV(A) under the new epoch");
    let refused = canary(&database, &world, &vault, current)
        .await
        .expect_err("the new epoch opened a copy keyed under the retired one");
    assert!(refused.contains("the key is wrong"), "{refused}");
    drop((node, world));
    let _ = std::fs::remove_dir_all(&root);
}
