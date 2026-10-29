//! The company audit trail: every admin change is recorded with who, what, to whom, when and why before it answers;
//! a refused change records nothing; a change whose record cannot be written does not happen; and a row changed
//! behind the server's back, in any column, breaks the chain where it was changed.

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use crate::world::{Cast, World};

/// Call as `token`, saying why.
async fn because(
    world: &World,
    method: Method,
    path: &str,
    token: &str,
    reason: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let headers = [
        ("authorization", format!("Bearer {token}")),
        ("vault-reason", reason.to_owned()),
    ];
    let (status, _, body) = world.call_with(method, path, &headers, body).await;
    (status, body)
}

async fn trail(world: &World, company: &str, token: &str) -> Vec<Value> {
    let (status, body) = world
        .call(
            Method::GET,
            &format!("/api/companies/{company}/audit"),
            Some(token),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["events"].as_array().expect("events").clone()
}

/// The newest `count` events as `(action, target, reason, actor)`, oldest of them first.
async fn newest(
    world: &World,
    company: &str,
    token: &str,
    count: usize,
) -> Vec<(String, String, String, String)> {
    let events = trail(world, company, token).await;
    let mut picked: Vec<_> = events
        .iter()
        .take(count)
        .map(|event| {
            let text = |name: &str| event[name].as_str().expect(name).to_owned();
            (
                text("action"),
                text("target"),
                text("reason"),
                text("actor"),
            )
        })
        .collect();
    picked.reverse();
    picked
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn every_admin_change_is_recorded_with_who_what_to_whom_and_why_and_a_refused_one_is_not() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let a = cast.company_a.as_str();
    let before = trail(&world, a, &cast.owner_a).await.len();
    let (status, body) = because(
        &world,
        Method::POST,
        &format!("/api/companies/{a}/members"),
        &cast.admin_a,
        "a new hire",
        Some(json!({ "email": "hire@alpha.test", "name": "Hire", "role": "member" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let hire = body["account_id"].as_str().expect("an id").to_owned();
    let member_path = format!("/api/companies/{a}/members/{hire}");
    let (status, body) = because(
        &world,
        Method::PATCH,
        &member_path,
        &cast.admin_a,
        "promoted",
        Some(json!({ "role": "admin" })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let (status, body) = because(
        &world,
        Method::POST,
        &format!("{member_path}/enrolment"),
        &cast.owner_a,
        "first sign-in",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let (status, body) = because(
        &world,
        Method::POST,
        &format!("/api/companies/{a}/groups"),
        &cast.admin_a,
        "new team",
        Some(json!({ "kind": "team", "name": "Payments", "parent": cast.department_a })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let group = body["id"].as_str().expect("an id").to_owned();
    let group_path = format!("/api/companies/{a}/groups/{group}");
    assert_eq!(
        because(
            &world,
            Method::PATCH,
            &group_path,
            &cast.admin_a,
            "renamed",
            Some(json!({ "name": "Billing" }))
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    let (status, body) = because(
        &world,
        Method::POST,
        &format!("{group_path}/members"),
        &cast.admin_a,
        "joins billing",
        Some(json!({ "account_id": hire })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    assert_eq!(
        because(
            &world,
            Method::DELETE,
            &format!("{group_path}/members/{hire}"),
            &cast.admin_a,
            "leaves billing",
            None
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        because(
            &world,
            Method::DELETE,
            &group_path,
            &cast.admin_a,
            "team closed",
            None
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        because(
            &world,
            Method::DELETE,
            &member_path,
            &cast.owner_a,
            "left the company",
            None
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        because(
            &world,
            Method::POST,
            &format!("/api/companies/{a}/suspend"),
            &cast.platform,
            "unpaid",
            None
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        because(
            &world,
            Method::POST,
            &format!("/api/companies/{a}/resume"),
            &cast.platform,
            "paid",
            None
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    // Refused: a member manages nothing, so nothing is recorded.
    let (status, _) = because(
        &world,
        Method::POST,
        &format!("/api/companies/{a}/members"),
        &cast.member_a,
        "sneaky",
        Some(json!({ "email": "x@alpha.test", "name": "X", "role": "member" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (admin, owner) = (cast.admin_a_id.clone(), cast.owner_a_id.clone());
    let platform = newest(&world, a, &cast.owner_a, 1).await[0].3.clone();
    let row = |action: &str, target: &str, reason: &str, actor: &str| {
        (
            action.to_owned(),
            target.to_owned(),
            reason.to_owned(),
            actor.to_owned(),
        )
    };
    assert_eq!(
        newest(&world, a, &cast.owner_a, 11).await,
        vec![
            row("member.add", &hire, "a new hire", &admin),
            row("member.change", &hire, "promoted", &admin),
            row("enrolment.issue", &hire, "first sign-in", &owner),
            row("group.create", &group, "new team", &admin),
            row("group.rename", &group, "renamed", &admin),
            row(
                "group.member.add",
                &format!("{group}:{hire}"),
                "joins billing",
                &admin
            ),
            row(
                "group.member.remove",
                &format!("{group}:{hire}"),
                "leaves billing",
                &admin
            ),
            row("group.delete", &group, "team closed", &admin),
            row("member.remove", &hire, "left the company", &owner),
            row("company.suspend", a, "unpaid", &platform),
            row("company.resume", a, "paid", &platform),
        ]
    );
    assert_eq!(
        trail(&world, a, &cast.owner_a).await.len(),
        before + 11,
        "exactly one event per change, none for the refusal"
    );
    assert_ne!(platform, owner, "the platform admin acted, not the owner");
    // The chain over these changes holds: every stored line is the one its columns rebuild.
    let (status, verified) = world
        .call(
            Method::GET,
            &format!("/api/companies/{a}/audit/verify"),
            Some(&cast.owner_a),
            None,
        )
        .await;
    assert_eq!(
        (status, verified["intact"].as_bool()),
        (StatusCode::OK, Some(true)),
        "{verified}"
    );
    // Another company's trail holds none of it.
    let b = trail(&world, &cast.company_b, &cast.admin_b).await;
    assert!(b.iter().all(|event| event["target"] != hire), "{b:?}");
    // A reason is text a person wrote, so control characters and an essay are refused before anything happens, also
    // when they arrive percent-encoded, and so is an encoding that is not UTF-8.
    let encoded_essay = "%78".repeat(501);
    for reason in [
        "tab\there",
        &"x".repeat(501),
        "line%0Abreak",
        "%FF",
        &encoded_essay,
    ] {
        let (status, body) = because(
            &world,
            Method::POST,
            &format!("/api/companies/{a}/groups"),
            &cast.admin_a,
            reason,
            Some(json!({ "kind": "department", "name": "Nope" })),
        )
        .await;
        assert_eq!(
            (status, body["error_code"].as_str()),
            (StatusCode::BAD_REQUEST, Some("invalid_request")),
            "{reason:?}"
        );
    }
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn the_chain_holds_and_names_the_row_changed_behind_the_servers_back() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let a = cast.company_a.as_str();
    let verify = |token: String| {
        let world = &world;
        async move {
            let (status, body) = world
                .call(
                    Method::GET,
                    &format!("/api/companies/{a}/audit/verify"),
                    Some(&token),
                    None,
                )
                .await;
            assert_eq!(status, StatusCode::OK, "{body}");
            body
        }
    };
    let held = verify(cast.owner_a.clone()).await;
    assert_eq!(held["intact"], true, "{held}");
    let events = trail(&world, a, &cast.owner_a).await.len();
    assert!(
        events >= 3,
        "the cast's own changes are on the trail: {events}"
    );
    let changed = world
        .database
        .run_at(
            &world.place,
            "UPDATE audit_events SET reason = 'nothing to see' WHERE company = $company AND seq = 2;",
            vec![("company".to_owned(), a.into())],
        )
        .await;
    assert!(changed.is_ok(), "{changed:?}");
    let broken = verify(cast.owner_a.clone()).await;
    assert_eq!(
        (broken["intact"].as_bool(), broken["broken_at"].as_i64()),
        (Some(false), Some(2)),
        "{broken}"
    );
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn a_change_whose_record_cannot_be_written_does_not_happen() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let a = cast.company_a.as_str();
    world
        .database
        .run_at(&world.place, "DROP TABLE audit_events;", vec![])
        .await
        .expect("the trail's table is gone in this world");
    let (status, body) = world
        .call(
            Method::POST,
            &format!("/api/companies/{a}/members"),
            Some(&cast.admin_a),
            Some(
                json!({ "email": "unrecorded@alpha.test", "name": "Unrecorded", "role": "member" }),
            ),
        )
        .await;
    assert_eq!(
        (status, body["error_code"].as_str()),
        (StatusCode::SERVICE_UNAVAILABLE, Some("unavailable")),
        "{body}"
    );
    let (status, members) = world
        .call(
            Method::GET,
            &format!("/api/companies/{a}/members"),
            Some(&cast.owner_a),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        members["members"]
            .as_array()
            .expect("members")
            .iter()
            .all(|m| m["account"]["email"] != "unrecorded@alpha.test"),
        "the member was not added: {members}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "needs TESSARIDB_BIN"]
async fn changes_to_one_company_at_once_are_each_recorded_once_in_one_chain() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let a = cast.company_a.as_str();
    let path = format!("/api/companies/{a}/groups");
    let mut created = 0_usize;
    for round in 0..3 {
        let create = |name: String| {
            let (world, path, token) = (&world, &path, &cast.admin_a);
            async move {
                world
                    .call(
                        Method::POST,
                        path,
                        Some(token),
                        Some(json!({ "kind": "department", "name": name })),
                    )
                    .await
            }
        };
        let answers = tokio::join!(
            create(format!("D{round}a")),
            create(format!("D{round}b")),
            create(format!("D{round}c")),
            create(format!("D{round}d"))
        );
        for (status, body) in [answers.0, answers.1, answers.2, answers.3] {
            // The loser of a race on the company's head is refused as undecided, never recorded out of chain.
            assert!(
                status == StatusCode::CREATED || status == StatusCode::SERVICE_UNAVAILABLE,
                "{status} {body}"
            );
            if status == StatusCode::CREATED {
                created = created.saturating_add(1);
            }
        }
    }
    assert!(created > 0, "some changes went through");
    let events = trail(&world, a, &cast.owner_a).await;
    let recorded = events
        .iter()
        .filter(|event| event["action"] == "group.create" && event["reason"] == "")
        .count();
    // The cast created two groups itself, with no reason.
    assert_eq!(
        recorded,
        created.saturating_add(2),
        "one event per change that happened"
    );
    let (status, verified) = world
        .call(
            Method::GET,
            &format!("/api/companies/{a}/audit/verify"),
            Some(&cast.owner_a),
            None,
        )
        .await;
    assert_eq!(
        (status, verified["intact"].as_bool()),
        (StatusCode::OK, Some(true)),
        "{verified}"
    );
}
