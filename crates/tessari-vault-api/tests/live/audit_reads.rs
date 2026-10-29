//! C8 for what SG3 built: every decryption bundle served (a record, a file, a key set, a group key), every share
//! and every recipient change is on the company's trail with who, what and to whom before it answers, and a read
//! whose record cannot be written is refused without the bundle.

use axum::http::{Method, StatusCode};
use serde_json::Value;

use crate::files::get_part;
use crate::shares::{into_team, keyed, record_path, shares, to_group};
use crate::world::{Cast, World, sealed_record};

const TEAM_KEY: &str = "a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1";

/// The reads a member makes of what they hold that answer JSON: their key set, their team's key and a record.
fn reads(cast: &Cast) -> [String; 3] {
    let a = &cast.company_a;
    [
        format!("/api/companies/{a}/keys/me"),
        format!("/api/companies/{a}/groups/{}/key", cast.team_a),
        record_path(cast, &cast.record_a),
    ]
}

/// The read of the record's file, which answers its bytes.
fn file_read(cast: &Cast) -> String {
    format!(
        "{}/files/{}?offset=0&limit=1",
        record_path(cast, &cast.record_a),
        cast.file_a
    )
}

async fn trail(world: &World, cast: &Cast) -> Vec<Value> {
    let (status, body) = world
        .call(
            Method::GET,
            &format!("/api/companies/{}/audit", cast.company_a),
            Some(&cast.owner_a),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["events"].as_array().expect("events").clone()
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn every_bundle_served_share_and_recipient_change_is_on_the_trail() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    into_team(&world, &cast, &cast.member_a_id).await;
    keyed(&world, &cast, &cast.team_a, TEAM_KEY, &[&cast.member_a_key]).await;
    for path in reads(&cast) {
        let (status, body) = world
            .call(Method::GET, &path, Some(&cast.member_a), None)
            .await;
        assert_eq!(status, StatusCode::OK, "{path}: {body}");
    }
    let (status, _) = get_part(&world, &file_read(&cast), &cast.member_a).await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = world
        .call(
            Method::POST,
            &shares(&cast, &cast.record_a),
            Some(&cast.member_a),
            Some(to_group(&cast.team_a, TEAM_KEY, "read", 0x51)),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let (status, body) = world
        .call(
            Method::DELETE,
            &format!("{}/{}", shares(&cast, &cast.record_a), cast.team_a),
            Some(&cast.member_a),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");

    let events = trail(&world, &cast).await;
    let recorded = |action: &str, target: &str, actor: &str| {
        events.iter().any(|event| {
            event["action"] == action && event["target"] == target && event["actor"] == actor
        })
    };
    let member = cast.member_a_id.as_str();
    let owner = cast.owner_a_id.as_str();
    for (action, target, actor) in [
        ("keys.read", cast.member_a_key.as_str(), member),
        ("group_key.read", cast.team_a.as_str(), member),
        ("record.read", cast.record_a.as_str(), member),
        ("file.read", cast.file_a.as_str(), member),
        ("group_key.keep", cast.team_a.as_str(), owner),
    ] {
        assert!(
            recorded(action, target, actor),
            "{action} on {target} by {actor}: {events:?}"
        );
    }
    for action in ["record.share", "record.unshare"] {
        assert!(
            events.iter().any(|event| event["action"] == action
                && event["actor"] == member
                && event["target"]
                    .as_str()
                    .is_some_and(|target| target.contains(&cast.record_a))),
            "{action}: {events:?}"
        );
    }
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn a_bundle_whose_read_cannot_be_recorded_is_not_served() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    into_team(&world, &cast, &cast.member_a_id).await;
    keyed(&world, &cast, &cast.team_a, TEAM_KEY, &[&cast.member_a_key]).await;
    world
        .database
        .run_at(&world.place, "DROP TABLE audit_events;", vec![])
        .await
        .expect("the trail's table is gone in this world");
    for path in reads(&cast) {
        let (status, body) = world
            .call(Method::GET, &path, Some(&cast.member_a), None)
            .await;
        assert_eq!(
            (status, body["error_code"].as_str()),
            (StatusCode::SERVICE_UNAVAILABLE, Some("unavailable")),
            "{path}: {body}"
        );
        for part in ["body", "wrap", "wrapped"] {
            assert!(body.get(part).is_none(), "{path} served {part}: {body}");
        }
    }
    let (status, bytes) = get_part(&world, &file_read(&cast), &cast.member_a).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    let refused: Value = serde_json::from_slice(&bytes).expect("a refusal, not the file's bytes");
    assert_eq!(refused["error_code"], "unavailable");
}

/// The ids a page of opened records answers, and the cursor to the next page.
fn page_of(body: &Value) -> (Vec<String>, Option<String>) {
    let ids = body["records"]
        .as_array()
        .expect("records")
        .iter()
        .map(|opened| {
            assert!(
                opened["wrap"].is_string(),
                "a bundle, not a listing: {opened}"
            );
            opened["record"]["id"].as_str().expect("an id").to_owned()
        })
        .collect();
    (ids, body["next"].as_str().map(str::to_owned))
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn a_page_of_opened_records_puts_every_bundle_on_the_trail() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let records = format!("/api/companies/{}/records", cast.company_a);
    for _ in 0..2 {
        let (status, body) = world
            .call(
                Method::POST,
                &records,
                Some(&cast.member_a),
                Some(sealed_record()),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
    }
    let reads_before = trail(&world, &cast)
        .await
        .iter()
        .filter(|event| event["action"] == "record.read")
        .count();
    let (status, first) = world
        .call(
            Method::GET,
            &format!("{records}?opened=true&limit=2"),
            Some(&cast.member_a),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{first}");
    let (mut seen, next) = page_of(&first);
    assert_eq!(seen.len(), 2);
    let next = next.expect("a third record waits on the next page");
    let (status, second) = world
        .call(
            Method::GET,
            &format!("{records}?opened=true&limit=2&after={next}"),
            Some(&cast.member_a),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{second}");
    let (rest, last) = page_of(&second);
    assert_eq!(last, None);
    seen.extend(rest);
    // Every record the member reaches, once each, and every one of them on the trail as its own read.
    let (_, listed) = world
        .call(Method::GET, &records, Some(&cast.member_a), None)
        .await;
    let mut expected: Vec<String> = listed["records"]
        .as_array()
        .expect("records")
        .iter()
        .map(|item| item["id"].as_str().expect("an id").to_owned())
        .collect();
    expected.sort();
    let mut got = seen.clone();
    got.sort();
    assert_eq!(got, expected);
    assert_eq!(expected.len(), 3);
    let reads_after = trail(&world, &cast)
        .await
        .iter()
        .filter(|event| event["action"] == "record.read")
        .count();
    assert_eq!(
        reads_after,
        reads_before + 3,
        "one trail row per bundle served"
    );
    // The owner reaches none of the member's records, so their page is empty.
    let (status, owners) = world
        .call(
            Method::GET,
            &format!("{records}?opened=true"),
            Some(&cast.owner_a),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{owners}");
    assert_eq!(page_of(&owners), (Vec::new(), None));
}
