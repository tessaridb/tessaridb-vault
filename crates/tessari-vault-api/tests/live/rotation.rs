//! Rotating the service key: every company vault moves to the ring's current epoch with its records intact, a move
//! cut short is finished by the next run, and a company whose keys are moving refuses requests without presenting a
//! key.

use axum::http::{Method, StatusCode};
use tessari_vault_infrastructure::tessaridb::{Answer, Value};

use crate::world::{Cast, World, older_epoch_key};

/// The ring after an operator added epoch 2 to a server that held epoch 1 alone.
fn added_epoch() -> String {
    format!("1 {}\n2 {}\n", older_epoch_key(), "cd".repeat(32))
}

fn bound(name: &str, value: &str) -> (String, Value) {
    (name.to_owned(), Value::from(value))
}

/// The company row's `key_epoch` and `key_moving`.
async fn row(world: &World, company: &str) -> (Option<Value>, Option<Value>) {
    let answers = world
        .database
        .run_at(
            &world.place,
            "RETURN { epoch: (GET groups:$c).key_epoch, moving: (GET groups:$c).key_moving };",
            vec![bound("c", company)],
        )
        .await
        .expect("the company row reads back");
    match answers.last() {
        Some(Answer::Value {
            value: Value::Object(fields),
            ..
        }) => (fields.get("epoch").cloned(), fields.get("moving").cloned()),
        other => panic!("not one row: {other:?}"),
    }
}

const PROBE: &str = "REVEAL body FROM {vault}:'rotation-probe';";

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn a_rotation_moves_every_company_vault_to_the_current_epoch_and_keeps_its_records() {
    let world = World::on_ring(&older_epoch_key()).await;
    let cast = Cast::new(&world).await;
    let vault = format!("records_{}", cast.company_a);
    world
        .database
        .run_presenting(
            &world.place,
            &format!("CREATE {vault}:'rotation-probe' = {{ body: 'sealed-before-rotation' }};"),
            Vec::new(),
            &vault,
            world
                .vault_keys
                .company(1, &cast.company_a)
                .expect("KV(A) under epoch 1"),
        )
        .await
        .expect("a body sealed under epoch 1");
    let (later, keys) = world.storage_on(&added_epoch());
    let rotated = later.rotation.rotate().await.expect("the rotation runs");
    assert_eq!(
        (rotated.moved, rotated.repaired, rotated.stranded.len()),
        (2, 0, 0),
        "both companies moved"
    );
    for company in [&cast.company_a, &cast.company_b] {
        assert_eq!(
            row(&world, company).await,
            (Some(Value::from(2_i64)), Some(Value::from(false))),
            "{company}"
        );
    }
    let probe = PROBE.replace("{vault}", &vault);
    let opened = world
        .database
        .run_presenting(
            &world.place,
            &probe,
            Vec::new(),
            &vault,
            keys.company(2, &cast.company_a)
                .expect("KV(A) under epoch 2"),
        )
        .await
        .expect("the current epoch's key opens the moved vault");
    assert!(
        matches!(opened.last(), Some(Answer::Value { value: Value::Object(fields), .. })
            if fields.get("body") == Some(&Value::from("sealed-before-rotation"))),
        "{opened:?}"
    );
    let refused = world
        .database
        .run_presenting(
            &world.place,
            &probe,
            Vec::new(),
            &vault,
            keys.company(1, &cast.company_a)
                .expect("KV(A) under epoch 1"),
        )
        .await
        .expect_err("the old epoch's key still opens the moved vault");
    assert!(
        refused.to_string().contains("the key is wrong"),
        "{refused:?}"
    );
    let again = later.rotation.rotate().await.expect("a second run");
    assert_eq!(
        (again.moved, again.repaired, again.stranded.len()),
        (0, 0, 0),
        "nothing left to move"
    );
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn a_move_cut_short_refuses_requests_and_is_finished_by_the_next_rotation() {
    let world = World::on_ring(&older_epoch_key()).await;
    let cast = Cast::new(&world).await;
    let (later, keys) = world.storage_on(&added_epoch());
    // The run that was cut short: company A marked moving and its vault re-wrapped, its row never updated.
    world
        .database
        .run_at(
            &world.place,
            "UPDATE groups:$c SET key_moving = true;",
            vec![bound("c", &cast.company_a)],
        )
        .await
        .expect("A is marked moving");
    world
        .database
        .change_keyed_vault(
            &world.place,
            &format!("records_{}", cast.company_a),
            &keys
                .company(1, &cast.company_a)
                .expect("KV(A) under epoch 1"),
            &keys
                .company(2, &cast.company_a)
                .expect("KV(A) under epoch 2"),
        )
        .await
        .expect("A's vault is re-wrapped");
    let (status, body) = world
        .call(
            Method::GET,
            &format!("/api/companies/{}/records", cast.company_a),
            Some(&cast.owner_a),
            None,
        )
        .await;
    assert_eq!(
        (status, body["error_code"].as_str()),
        (StatusCode::CONFLICT, Some("keys_moving")),
        "{body}"
    );
    let rotated = later.rotation.rotate().await.expect("the rotation runs");
    assert_eq!(
        (rotated.moved, rotated.repaired, rotated.stranded.len()),
        (1, 1, 0),
        "B moved, A finished"
    );
    assert_eq!(
        row(&world, &cast.company_a).await,
        (Some(Value::from(2_i64)), Some(Value::from(false)))
    );
}
