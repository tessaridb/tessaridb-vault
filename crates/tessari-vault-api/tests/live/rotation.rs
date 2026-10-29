//! Rotating the service key: every company vault moves to the ring's current epoch with its records intact, a move
//! cut short is finished by the next run, and a company whose keys are moving refuses requests without presenting a
//! key.

use axum::http::{Method, StatusCode};
use tessari_vault_infrastructure::custody::TotpSeal;
use tessari_vault_infrastructure::tessaridb::{Answer, Value};

use tessari_vault_core::totp;

use crate::enrolment::{base32, begin, complete, issue};
use crate::world::{Cast, World, add_member, older_epoch_key};

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

/// Enrol a new member of company A with a password and an authenticator, so a sealed seed exists.
async fn enrol(world: &World, cast: &Cast, email: &str) -> String {
    let account = add_member(world, &cast.company_a, &cast.admin_a, email, "member").await;
    let token = issue(world, &cast.admin_a, &cast.company_a, &account)
        .await
        .1["token"]
        .as_str()
        .expect("a token")
        .to_owned();
    let secret = begin(world, &token).await.1["secret"]
        .as_str()
        .expect("a secret")
        .to_owned();
    let step = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("after 1970")
        .as_secs()
        / 30;
    let code = format!("{:06}", totp::code_at(&base32(&secret), step));
    let (status, body) = complete(world, &token, "a passphrase of my own", &code).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    account
}

/// The epoch each enrolled credential's sealed seed names.
async fn seed_epochs(world: &World) -> Vec<Option<u32>> {
    let answers = world
        .database
        .run_at(&world.place, "SELECT totp FROM credentials;", Vec::new())
        .await
        .expect("the credentials read back");
    let Some(Answer::Records { records, .. }) = answers.last() else {
        panic!("not records: {answers:?}");
    };
    records
        .iter()
        .map(|(_, record)| match record {
            Value::Object(fields) => match fields.get("totp") {
                Some(Value::String(text)) => {
                    let bytes: Vec<u8> = (0..text.len())
                        .step_by(2)
                        .filter_map(|at| {
                            u8::from_str_radix(text.get(at..at.saturating_add(2))?, 16).ok()
                        })
                        .collect();
                    TotpSeal::epoch_of(&bytes)
                }
                _ => None,
            },
            _ => None,
        })
        .collect()
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
    enrol(&world, &cast, "seeded@alpha.test").await;
    let enrolled = seed_epochs(&world).await;
    assert!(!enrolled.is_empty() && enrolled.iter().all(|epoch| *epoch == Some(1)));
    let (later, keys) = world.storage_on(&added_epoch());
    let before = later.rotation.remaining().await.expect("the counts");
    assert_eq!(
        (before.companies, before.verifiers, before.seeds),
        (vec![(1, 2)], vec![(1, 1)], vec![(1, 1)]),
        "everything on epoch 1"
    );
    let rotated = later.rotation.rotate().await.expect("the rotation runs");
    assert_eq!(
        (rotated.moved, rotated.repaired, rotated.stranded.len()),
        (2, 0, 0),
        "both companies moved"
    );
    let after = later.rotation.remaining().await.expect("the counts");
    assert_eq!(
        (after.companies, after.verifiers, after.seeds),
        (vec![(2, 2)], vec![(1, 1)], vec![(2, 1)]),
        "the verifier moves only when its member signs in"
    );
    assert_eq!(
        (rotated.seeds_resealed, rotated.seeds_unopened),
        (enrolled.len(), 0),
        "every seed sealed again"
    );
    assert!(
        seed_epochs(&world)
            .await
            .iter()
            .all(|epoch| *epoch == Some(2)),
        "every seed now names epoch 2"
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
        (
            again.moved,
            again.repaired,
            again.stranded.len(),
            again.seeds_resealed
        ),
        (0, 0, 0, 0),
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

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn the_platform_admin_resets_a_sign_in_resting_on_a_retired_epoch_and_no_one_else_may() {
    use crate::world::call_app;

    let world = World::on_ring(&older_epoch_key()).await;
    let cast = Cast::new(&world).await;
    let member = enrol(&world, &cast, "stranded@alpha.test").await;
    // A member of both companies, so the reset joins both trails.
    let in_b = add_member(
        &world,
        &cast.company_b,
        &cast.admin_b,
        "stranded@alpha.test",
        "member",
    )
    .await;
    assert_eq!(in_b, member, "one account in two companies");
    let platform = world.platform_admin().await;
    let reset = format!("/api/accounts/{member}/sign-in/reset");
    // While the ring still holds epoch 1 the member can sign in and move their password themselves.
    let holding = world.app_on(&added_epoch());
    let (status, body) =
        call_app(holding.clone(), Method::POST, &reset, Some(&platform), None).await;
    assert_eq!(
        (status, body["error_code"].as_str()),
        (StatusCode::CONFLICT, Some("sign_in_current")),
        "{body}"
    );
    // Epoch 1 has left the ring: only the platform admin may reset, and the link that comes back enrols them again.
    let retired = world.app_on(&format!("2 {}\n", "cd".repeat(32)));
    let (status, body) = call_app(
        retired.clone(),
        Method::POST,
        &reset,
        Some(&cast.admin_a),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "a company admin: {body}");
    let (status, body) =
        call_app(retired.clone(), Method::POST, &reset, Some(&platform), None).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let token = body["token"].as_str().expect("a link").to_owned();
    let (status, body) = call_app(
        retired.clone(),
        Method::POST,
        "/api/enrolment/begin",
        None,
        Some(serde_json::json!({ "token": token })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "the link begins an enrolment: {body}"
    );
    for company in [&cast.company_a, &cast.company_b] {
        let trail = world
            .database
            .run_at(
                &world.place,
                "SELECT action, target FROM audit_events WHERE company = $c AND action = 'sign_in.reset';",
                vec![bound("c", company)],
            )
            .await
            .expect("the trail reads back");
        assert!(
            matches!(trail.last(), Some(Answer::Records { records, .. }) if records.len() == 1),
            "one reset on the trail of {company}: {trail:?}"
        );
    }
}
