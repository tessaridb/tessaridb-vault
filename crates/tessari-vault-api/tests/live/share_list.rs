//! A record's shares, listed to whoever may revise it: the author's key and each share's target, right and key, which
//! is exactly the set of wraps a revision must carry; a reader is refused and anyone it does not reach finds nothing.

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use crate::records::encoded;
use crate::shares::{record_path, revision, shares, to_member};
use crate::world::{Cast, World};

async fn listed(world: &World, cast: &Cast, token: &str) -> (StatusCode, Value) {
    world
        .call(
            Method::GET,
            &shares(cast, &cast.record_a),
            Some(token),
            None,
        )
        .await
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn a_records_shares_are_listed_to_its_editors_and_name_every_wrap_a_revision_keeps() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    for (account, key, right) in [
        (&cast.admin_a_id, &cast.admin_a_key, "edit"),
        (&cast.owner_a_id, &cast.owner_a_key, "read"),
    ] {
        let (status, body) = world
            .call(
                Method::POST,
                &shares(&cast, &cast.record_a),
                Some(&cast.member_a),
                Some(to_member(account, key, right, 5)),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
    }
    let mut expected = vec![
        json!({ "kind": "member", "target": cast.admin_a_id, "right": "edit", "key_id": cast.admin_a_key }),
        json!({ "kind": "member", "target": cast.owner_a_id, "right": "read", "key_id": cast.owner_a_key }),
    ];
    expected.sort_by_key(|share| share["key_id"].as_str().map(str::to_owned));
    for token in [&cast.member_a, &cast.admin_a] {
        let (status, body) = listed(&world, &cast, token).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(
            body["author"],
            json!({ "account": cast.member_a_id, "key_id": cast.member_a_key })
        );
        let mut shares = body["shares"].as_array().expect("shares").clone();
        shares.sort_by_key(|share| share["key_id"].as_str().map(str::to_owned));
        assert_eq!(shares, expected, "{body}");
    }
    let (status, body) = listed(&world, &cast, &cast.owner_a).await;
    assert_eq!(
        (status, body["error_code"].as_str()),
        (StatusCode::FORBIDDEN, Some("forbidden")),
        "a reader: {body}"
    );
    let (status, body) = listed(&world, &cast, &cast.admin_b).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "another company: {body}");

    // The editor's revision wraps to every listed recipient but its own key, and the server takes exactly that.
    let (_, body) = listed(&world, &cast, &cast.admin_a).await;
    let own = format!("member:{}", cast.admin_a_key);
    let mut recipients = vec![format!(
        "member:{}",
        body["author"]["key_id"].as_str().expect("a key")
    )];
    for share in body["shares"].as_array().expect("shares") {
        recipients.push(format!(
            "{}:{}",
            share["kind"].as_str().expect("a kind"),
            share["key_id"].as_str().expect("a key")
        ));
    }
    let wraps: Vec<Value> = recipients
        .into_iter()
        .filter(|name| *name != own)
        .map(|name| json!({ "recipient": name, "wrap": encoded(&[8; 82]) }))
        .collect();
    let (status, body) = world
        .call(
            Method::PUT,
            &record_path(&cast, &cast.record_a),
            Some(&cast.admin_a),
            Some(revision(1, &cast.admin_a_key, wraps)),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "the listed set is the set a revision keeps: {body}"
    );
}
