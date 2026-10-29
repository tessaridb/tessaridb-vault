//! Member keys and the vouching chain: each member keeps one key set per company, its private half wrapped in the
//! browser and kept in the company's vault; the chain grows one statement at a time from an owner's genesis.

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use crate::records::{count, encoded, files_under, signature};
use crate::world::{Cast, World};

/// A member's key set as the browser registers one; `seed` makes each set's bytes its own.
pub(crate) fn key_set(key_id: &str, seed: u8) -> Value {
    json!({
        "key_id": key_id,
        "agreement_public": encoded(&[seed; 32]),
        "signing_public": encoded(&[seed.wrapping_add(1); 32]),
        "wrapped": encoded(&[seed.wrapping_add(2); 120]),
        "kdf": { "salt": encoded(&[seed; 16]), "memory_kib": 47_104, "passes": 2, "lanes": 1 },
    })
}

fn statement(seq: u64, seed: u8) -> Value {
    json!({ "seq": seq, "bytes": encoded(&[seed; 140]), "signature": signature(seed) })
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn a_member_keeps_one_key_set_per_company_and_reads_it_back() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let mine = format!("/api/companies/{}/keys/me", cast.company_a);
    // The cast registered member A's set; it reads back whole, wrapped half included.
    let (status, read) = world
        .call(Method::GET, &mine, Some(&cast.member_a), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{read}");
    assert_eq!(read, key_set(&cast.member_a_key, 2));

    let (status, again) = world
        .call(
            Method::PUT,
            &mine,
            Some(&cast.member_a),
            Some(key_set(&"77".repeat(16), 7)),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{again}");
    assert_eq!(again["error_code"], "keys_exist");

    let fresh = crate::world::add_member(
        &world,
        &cast.company_a,
        &cast.owner_a,
        "fresh@alpha.test",
        "member",
    )
    .await;
    let fresh = world.token(&fresh).await;
    let (status, missing) = world.call(Method::GET, &mine, Some(&fresh), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{missing}");
    for weak in [
        json!({ "memory_kib": 1_024, "passes": 2, "lanes": 1 }),
        json!({ "memory_kib": 47_104, "passes": 0, "lanes": 1 }),
        json!({ "memory_kib": 47_104, "passes": 2, "lanes": 9 }),
    ] {
        let mut set = key_set(&"88".repeat(16), 8);
        set["kdf"]["memory_kib"] = weak["memory_kib"].clone();
        set["kdf"]["passes"] = weak["passes"].clone();
        set["kdf"]["lanes"] = weak["lanes"].clone();
        let (status, refused) = world
            .call(Method::PUT, &mine, Some(&fresh), Some(set))
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{weak}: {refused}");
    }
    // A key id already registered in the company is somebody else's.
    let (status, taken) = world
        .call(
            Method::PUT,
            &mine,
            Some(&fresh),
            Some(key_set(&cast.member_a_key, 9)),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{taken}");
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn the_key_directory_lists_the_companys_public_keys_only() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let directory = format!("/api/companies/{}/keys", cast.company_a);
    let (status, listed) = world
        .call(Method::GET, &directory, Some(&cast.owner_a), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{listed}");
    let keys = listed["keys"].as_array().expect("a list");
    let member = keys
        .iter()
        .find(|key| key["account"] == cast.member_a_id.as_str())
        .expect("member A's keys are listed");
    let set = key_set(&cast.member_a_key, 2);
    assert_eq!(member["key_id"], set["key_id"]);
    assert_eq!(member["agreement_public"], set["agreement_public"]);
    assert_eq!(member["signing_public"], set["signing_public"]);
    assert!(
        member.get("wrapped").is_none() && member.get("kdf").is_none(),
        "the directory carries a private half: {member}"
    );
    assert!(
        keys.iter()
            .all(|key| key["account"] != cast.admin_b_id.as_str()),
        "another company's member is listed"
    );
    let (status, _) = world
        .call(Method::GET, &directory, Some(&cast.admin_b), None)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn the_chain_grows_one_statement_at_a_time_from_an_owners_genesis() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let chain = format!("/api/companies/{}/statements", cast.company_a);
    let post = |token: String, body: Value| {
        let world = &world;
        let chain = chain.clone();
        async move {
            world
                .call(Method::POST, &chain, Some(&token), Some(body))
                .await
        }
    };
    let mut genesis = statement(1, 1);
    genesis["owner_signing"] = json!(encoded(&[3; 32]));

    let (status, refused) = post(cast.member_a.clone(), genesis.clone()).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a member began the chain: {refused}"
    );
    let (status, refused) = post(cast.admin_a.clone(), genesis.clone()).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "an admin began the chain: {refused}"
    );
    let (status, gap) = post(cast.owner_a.clone(), statement(2, 2)).await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "a statement before the genesis: {gap}"
    );
    assert_eq!(gap["error_code"], "stale_chain");
    let (status, unpinned) = post(cast.owner_a.clone(), statement(1, 1)).await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "a genesis with no pin: {unpinned}"
    );
    let (status, body) = post(cast.owner_a.clone(), genesis.clone()).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let (status, twice) = post(cast.owner_a.clone(), genesis.clone()).await;
    assert_eq!(status, StatusCode::CONFLICT, "a second genesis: {twice}");
    let mut pinned_later = statement(2, 2);
    pinned_later["owner_signing"] = json!(encoded(&[4; 32]));
    let (status, refused) = post(cast.owner_a.clone(), pinned_later).await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "a pin after the genesis: {refused}"
    );
    let (status, body) = post(cast.admin_a.clone(), statement(2, 2)).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let (status, refused) = post(cast.member_a.clone(), statement(3, 3)).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a member extended the chain: {refused}"
    );
    let (status, ahead) = post(cast.owner_a.clone(), statement(4, 4)).await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "a statement past the end: {ahead}"
    );

    let (status, read) = world
        .call(Method::GET, &chain, Some(&cast.member_a), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{read}");
    assert_eq!(read["owner_signing"], encoded(&[3; 32]));
    let statements = read["statements"].as_array().expect("a list");
    assert_eq!(statements.len(), 2, "{read}");
    for (at, seed) in [(0_usize, 1_u8), (1, 2)] {
        let expected = statement(u64::from(seed), seed);
        assert_eq!(statements[at]["seq"], expected["seq"]);
        assert_eq!(statements[at]["bytes"], expected["bytes"]);
        assert_eq!(statements[at]["signature"], expected["signature"]);
    }
    let (status, _) = world
        .call(
            Method::GET,
            &format!("/api/companies/{}/statements", cast.company_a),
            Some(&cast.admin_b),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn a_wrapped_key_set_is_absent_from_the_backup_and_the_store_directory() {
    let directory = std::env::temp_dir().join(format!("vault-keys-scan-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    let world = World::on_disk(&directory).await;
    let cast = Cast::new(&world).await;
    let canary = b"keys-scan-canary-wrapped-5e81".to_vec();
    let mut set = key_set(&"99".repeat(16), 9);
    set["wrapped"] = json!(encoded(&canary));
    let fresh = crate::world::add_member(
        &world,
        &cast.company_a,
        &cast.owner_a,
        "scan@alpha.test",
        "member",
    )
    .await;
    let fresh = world.token(&fresh).await;
    let (status, body) = world
        .call(
            Method::PUT,
            &format!("/api/companies/{}/keys/me", cast.company_a),
            Some(&fresh),
            Some(set.clone()),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

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
    // The control is the set's public half, kept in the clear on its row, which the scan must find.
    let public = set["agreement_public"]
        .as_str()
        .expect("text")
        .as_bytes()
        .to_vec();
    assert!(
        count(&backup, &public) > 0,
        "the scan cannot see the key row in the backup"
    );
    assert!(
        in_files(&public) > 0,
        "the scan cannot see the key row in the store directory"
    );
    for needle in [canary.clone(), encoded(&canary).into_bytes()] {
        assert_eq!(
            count(&backup, &needle),
            0,
            "the wrapped set is in the backup"
        );
        assert_eq!(
            in_files(&needle),
            0,
            "the wrapped set is in the store directory"
        );
    }
    drop(world);
    let _ = std::fs::remove_dir_all(&directory);
}
