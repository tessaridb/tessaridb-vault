//! Deleting a company: an owner asks with a statement their own registered key signed, so the server cannot ask on
//! an owner's behalf, and nobody else may ask.

use std::time::{SystemTime, UNIX_EPOCH};

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};
use tessari_vault_core::company_drop::{Act, statement};

use crate::keys::key_set;
use crate::records::encoded;
use crate::statements::Signer;
use crate::world::{Cast, World, add_member};

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("after 1970")
        .as_secs()
}

/// A second owner of company A who keeps a key set whose signing key `signer` holds.
async fn signing_owner(world: &World, cast: &Cast, signer: &Signer) -> String {
    let account = add_member(
        world,
        &cast.company_a,
        &cast.owner_a,
        "second-owner@alpha.test",
        "owner",
    )
    .await;
    let token = world.token(&account).await;
    let mut keys = key_set(&"66".repeat(16), 6);
    keys["signing_public"] = Value::from(encoded(&signer.public()));
    let (status, body) = world
        .call(
            Method::PUT,
            &format!("/api/companies/{}/keys/me", cast.company_a),
            Some(&token),
            Some(keys),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    token
}

fn asked(company: &str, by: &Signer, at: u64) -> Value {
    json!({
        "at": at,
        "signature": encoded(&by.stored(&statement(Act::Request, company, at))),
    })
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn an_owner_asks_with_a_statement_their_own_key_signed_and_nobody_else_may() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let signer = Signer::new();
    let owner = signing_owner(&world, &cast, &signer).await;
    let drop = format!("/api/companies/{}/drop", cast.company_a);
    let a = cast.company_a.as_str();
    for (who, token, body, expected, code) in [
        (
            "an admin",
            &cast.admin_a,
            asked(a, &signer, now()),
            StatusCode::FORBIDDEN,
            Some("forbidden"),
        ),
        (
            "another key",
            &owner,
            asked(a, &Signer::new(), now()),
            StatusCode::CONFLICT,
            Some("drop_unsigned"),
        ),
        (
            "an hour old",
            &owner,
            asked(a, &signer, now().saturating_sub(3600)),
            StatusCode::CONFLICT,
            Some("drop_unsigned"),
        ),
        (
            "another company's statement",
            &owner,
            asked(&cast.company_b, &signer, now()),
            StatusCode::CONFLICT,
            Some("drop_unsigned"),
        ),
    ] {
        let (status, answer) = world
            .call(Method::POST, &drop, Some(token), Some(body))
            .await;
        assert_eq!(
            (status, answer["error_code"].as_str().map(str::to_owned)),
            (expected, code.map(str::to_owned)),
            "{who}: {answer}"
        );
    }
    let (status, answer) = world
        .call(
            Method::POST,
            &drop,
            Some(&owner),
            Some(asked(a, &signer, now())),
        )
        .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{answer}");
    assert_eq!(answer["state"], "requested", "{answer}");
    let (status, trail) = world
        .call(
            Method::GET,
            &format!("/api/companies/{a}/audit"),
            Some(&cast.owner_a),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{trail}");
    assert!(
        trail.to_string().contains("company.drop.request"),
        "the request is on the trail: {trail}"
    );
}

fn cancelling(company: &str, by: &Signer, at: u64) -> Value {
    json!({
        "at": at,
        "signature": encoded(&by.stored(&statement(Act::Cancel, company, at))),
    })
}

/// Where `company`'s deletion stands, as `caller` reads it.
async fn standing(world: &World, company: &str, caller: &str) -> (StatusCode, Value) {
    world
        .call(
            Method::GET,
            &format!("/api/companies/{company}/drop"),
            Some(caller),
            None,
        )
        .await
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn the_platform_admin_confirms_and_the_company_waits_read_only_until_an_owner_cancels() {
    use crate::world::sealed_record;

    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let signer = Signer::new();
    let owner = signing_owner(&world, &cast, &signer).await;
    let a = cast.company_a.as_str();
    let base = format!("/api/companies/{a}/drop");
    let (status, body) = world
        .call(
            Method::POST,
            &format!("/api/companies/{}/drop/confirm", cast.company_b),
            Some(&cast.platform),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "nothing asked for B: {body}");
    let (status, body) = standing(&world, a, &owner).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "no deletion open: {body}");
    let (status, body) = world
        .call(
            Method::POST,
            &base,
            Some(&owner),
            Some(asked(a, &signer, now())),
        )
        .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{body}");
    let (status, body) = standing(&world, a, &owner).await;
    assert_eq!(
        (status, body["state"].as_str()),
        (StatusCode::OK, Some("requested")),
        "{body}"
    );
    let (status, body) = world
        .call(
            Method::GET,
            "/api/company-drops",
            Some(&cast.platform),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["drops"],
        json!([{ "company": a, "state": "requested" }]),
        "the platform admin's queue: {body}"
    );
    let (status, body) = world
        .call(Method::GET, "/api/company-drops", Some(&owner), None)
        .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "the queue is the platform admin's: {body}"
    );
    // An owner may take back a request the platform admin has not confirmed yet, and ask again.
    let (status, body) = world
        .call(
            Method::POST,
            &format!("{base}/cancel"),
            Some(&owner),
            Some(cancelling(a, &signer, now())),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a request is cancelled before it is confirmed: {body}"
    );
    let (status, body) = standing(&world, a, &owner).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let (status, body) = world
        .call(
            Method::POST,
            &base,
            Some(&owner),
            Some(asked(a, &signer, now())),
        )
        .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{body}");
    let (status, body) = standing(&world, a, &cast.admin_a).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "an admin does not ask, nor read: {body}"
    );
    let (status, body) = standing(&world, cast.company_b.as_str(), &owner).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "another company's is not shown: {body}"
    );
    let confirm = format!("{base}/confirm");
    let (status, body) = world
        .call(Method::POST, &confirm, Some(&cast.owner_a), None)
        .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "an owner is not the second party: {body}"
    );
    let (status, body) = world
        .call(Method::POST, &confirm, Some(&cast.platform), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["state"], "scheduled", "{body}");
    let after = body["drop_after"].as_u64().expect("a time");
    assert!(
        after.abs_diff(now().saturating_add(604_800)) < 120,
        "seven days: {body}"
    );

    let records = format!("/api/companies/{a}/records");
    let (status, body) = world
        .call(Method::GET, &records, Some(&cast.member_a), None)
        .await;
    assert_eq!(status, StatusCode::OK, "reads go on: {body}");
    let (status, body) = world
        .call(
            Method::GET,
            &format!("/api/companies/{a}/keys/me"),
            Some(&cast.member_a),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "a member can still unlock: {body}");
    let (status, body) = world
        .call(
            Method::POST,
            &records,
            Some(&cast.member_a),
            Some(sealed_record()),
        )
        .await;
    assert_eq!(
        (status, body["error_code"].as_str()),
        (StatusCode::CONFLICT, Some("company_closing")),
        "{body}"
    );
    let (status, body) = standing(&world, a, &cast.platform).await;
    assert_eq!(
        (status, body["state"].as_str(), body["drop_after"].as_u64()),
        (StatusCode::OK, Some("scheduled"), Some(after)),
        "{body}"
    );
    let (status, body) = world
        .call(
            Method::POST,
            &base,
            Some(&owner),
            Some(asked(a, &signer, now())),
        )
        .await;
    assert_eq!(
        (status, body["error_code"].as_str()),
        (StatusCode::CONFLICT, Some("drop_scheduled")),
        "asking again does not take back a confirmation: {body}"
    );

    let cancel = format!("{base}/cancel");
    let (status, body) = world
        .call(
            Method::POST,
            &cancel,
            Some(&cast.admin_a),
            Some(cancelling(a, &signer, now())),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    let (status, body) = world
        .call(
            Method::POST,
            &cancel,
            Some(&owner),
            Some(asked(a, &signer, now())),
        )
        .await;
    assert_eq!(
        (status, body["error_code"].as_str()),
        (StatusCode::CONFLICT, Some("drop_unsigned")),
        "a request's signature does not cancel: {body}"
    );
    let (status, body) = world
        .call(
            Method::POST,
            &cancel,
            Some(&owner),
            Some(cancelling(a, &signer, now())),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["state"], "cancelled", "{body}");
    let (status, body) = standing(&world, a, &owner).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "closed by the cancel: {body}"
    );
    let (status, body) = world
        .call(
            Method::POST,
            &records,
            Some(&cast.member_a),
            Some(sealed_record()),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "writes resume: {body}");
    let (_, trail) = world
        .call(
            Method::GET,
            &format!("/api/companies/{a}/audit"),
            Some(&cast.owner_a),
            None,
        )
        .await;
    for action in ["company.drop.confirm", "company.drop.cancel"] {
        assert!(trail.to_string().contains(action), "{action} on the trail");
    }
}

/// What the bucket answers for the first part of `company`'s file `file`.
async fn part_zero(world: &World, company: &str, file: &str) -> String {
    let answers = world
        .database
        .run_at(
            &world.place,
            "READ record_files:$p;",
            vec![(
                "p".to_owned(),
                tessari_vault_infrastructure::tessaridb::Value::from(
                    format!("/{company}/{file}/0").as_str(),
                ),
            )],
        )
        .await;
    format!("{answers:?}")
}

/// The tables that hold a company's rows by its `company` field, which a deletion must leave empty of it.
const COMPANY_TABLES: [&str; 14] = [
    "groups",
    "memberships",
    "company_drops",
    "records",
    "record_types",
    "files",
    "member_keys",
    "chains",
    "statements",
    "group_keys",
    "shares",
    "recovery_keys",
    "escrows",
    "recovery_notices",
];

async fn count(world: &World, statement: &str, company: &str) -> usize {
    use tessari_vault_infrastructure::tessaridb::{Answer, Value as StoreValue};
    let answers = world
        .database
        .run_at(
            &world.place,
            statement,
            vec![("c".to_owned(), StoreValue::from(company))],
        )
        .await
        .expect("the store answers");
    match answers.last() {
        Some(Answer::Records { records, .. }) => records.len(),
        other => panic!("{statement}: {other:?}"),
    }
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn a_deletion_past_its_wait_is_carried_out_and_leaves_only_the_trail() {
    use crate::files::new_file;

    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let signer = Signer::new();
    let owner = signing_owner(&world, &cast, &signer).await;
    let a = cast.company_a.as_str();
    let (part, created) = new_file(&world, &cast, 100_000).await;
    assert_eq!(created["kept"], "bucket");
    let (status, body) = world
        .put_octets(
            &format!("{part}?offset=0"),
            &cast.member_a,
            vec![7; 100_000],
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let file = created["id"].as_str().expect("an id").to_owned();
    let before = part_zero(&world, a, &file).await;
    assert!(
        before.contains(&"7, ".repeat(8)),
        "the control: the part is in the bucket before the drop: {before}"
    );
    crate::group_keys::member_in_team(&world, &cast, &cast.member_a_id).await;
    let revisions = format!(
        "SELECT * FROM revisions WHERE record = '{}';",
        cast.record_a
    );
    let joined = format!("SELECT * FROM accounts:'{}'->in_group;", cast.member_a_id);
    let placed = format!("SELECT * FROM groups:'{}'->part_of;", cast.team_a);
    for (statement, what) in [
        (&revisions, "revisions"),
        (&joined, "a member's group edges"),
        (&placed, "the team's place"),
    ] {
        assert!(
            count(&world, statement, a).await > 0,
            "the control: {what} exist before the drop"
        );
    }
    let base = format!("/api/companies/{a}/drop");
    let (status, _) = world
        .call(
            Method::POST,
            &base,
            Some(&owner),
            Some(asked(a, &signer, now())),
        )
        .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let (status, _) = world
        .call(
            Method::POST,
            &format!("{base}/confirm"),
            Some(&cast.platform),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        world
            .storage
            .company_drops
            .drop_due()
            .await
            .expect("the run")
            .is_empty(),
        "nothing is due before its wait passes"
    );
    world
        .database
        .run_at(
            &world.place,
            "UPDATE company_drops:$c SET drop_after = 0;",
            vec![(
                "c".to_owned(),
                tessari_vault_infrastructure::tessaridb::Value::from(a),
            )],
        )
        .await
        .expect("the wait is over");
    let dropped = world
        .storage
        .company_drops
        .drop_due()
        .await
        .expect("the run");
    assert_eq!(dropped.len(), 1, "{dropped:?}");
    assert_eq!(dropped[0].as_str(), a);

    for table in COMPANY_TABLES {
        assert_eq!(
            count(
                &world,
                &format!("SELECT * FROM {table} WHERE company = $c;"),
                a
            )
            .await,
            0,
            "{table} still holds the company"
        );
    }
    for (statement, what) in [
        (&revisions, "revisions"),
        (&joined, "a member's group edges"),
        (&placed, "the team's place"),
    ] {
        assert_eq!(
            count(&world, statement, a).await,
            0,
            "{what} outlive the drop"
        );
    }
    assert!(
        count(
            &world,
            "SELECT * FROM audit_events WHERE company = $c AND action = 'company.drop';",
            a
        )
        .await
            == 1,
        "the drop is the trail's last word"
    );
    assert!(
        count(&world, "SELECT * FROM audit_events WHERE company = $c;", a).await > 1,
        "the trail outlives the rows"
    );
    let info = world
        .database
        .run_at(&world.place, "INFO FOR DATABASE;", Vec::new())
        .await
        .expect("the database describes itself");
    let described = format!("{info:?}");
    assert!(
        !described.contains(&format!("records_{a}")),
        "the vault is dropped"
    );
    assert!(
        described.contains(&format!("records_{}", cast.company_b)),
        "B's vault stays"
    );
    let after = part_zero(&world, a, &file).await;
    assert!(
        !after.contains(&"7, ".repeat(8)),
        "no part of A's file is left: {after}"
    );
    let (status, body) = world
        .call(
            Method::GET,
            &format!("/api/companies/{}/members", cast.company_b),
            Some(&cast.admin_b),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "B is untouched: {body}");
}
