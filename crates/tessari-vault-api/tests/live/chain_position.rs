//! A revision names the position of the company's chain its author signed at, and the server stores it only at the
//! chain's current length, so no revision lands back-dated to a position before its author's key was replaced.

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use crate::records::{encoded, revision, signature};
use crate::statements::{Growing, Says, Signer};
use crate::world::{Cast, World, fresh_record_id};

/// A record member A seals at chain position `chain_seq`.
fn sealed_at(chain_seq: u64) -> Value {
    json!({
        "id": fresh_record_id(),
        "record_type": "login",
        "key_id": "11".repeat(16),
        "body": encoded(b"sealed"),
        "signature": signature(7),
        "author_key": "22".repeat(16),
        "chain_seq": chain_seq,
        "wrap": encoded(&[9; 82]),
    })
}

/// Grows company A's chain by one owner-signed statement naming `subject`.
async fn grow(world: &World, cast: &Cast, growing: &mut Growing, owner: &Signer, subject: &str) {
    let says = Says {
        kind: 1,
        subject,
        agreement: &[1; 32],
        signing: &owner.public(),
    };
    let (mut body, after) = growing.next(&says, owner);
    if subject == "owner" {
        body["owner_signing"] = json!(encoded(&owner.public()));
    }
    let (status, answer) = world
        .call(
            Method::POST,
            &format!("/api/companies/{}/statements", cast.company_a),
            Some(&cast.owner_a),
            Some(body),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{answer}");
    growing.accepted(after);
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn a_revision_is_stored_only_at_the_chains_current_length() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let records = format!("/api/companies/{}/records", cast.company_a);
    let owner = Signer::new();
    let mut growing = Growing::new(&cast.company_a);
    grow(&world, &cast, &mut growing, &owner, "owner").await;

    for (behind_or_ahead, seq) in [("behind", 0), ("ahead", 2)] {
        let (status, refused) = world
            .call(
                Method::POST,
                &records,
                Some(&cast.member_a),
                Some(sealed_at(seq)),
            )
            .await;
        assert_eq!(status, StatusCode::CONFLICT, "{behind_or_ahead}: {refused}");
        assert_eq!(refused["error_code"], "stale_chain", "{behind_or_ahead}");
    }
    let (status, created) = world
        .call(
            Method::POST,
            &records,
            Some(&cast.member_a),
            Some(sealed_at(1)),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let path = format!("{records}/{}", created["id"].as_str().expect("an id"));
    let read_seq = || async {
        let (status, read) = world
            .call(Method::GET, &path, Some(&cast.member_a), None)
            .await;
        assert_eq!(status, StatusCode::OK, "{read}");
        read["chain_seq"].clone()
    };
    assert_eq!(read_seq().await, 1);

    // The chain moves on: a revision sealed against the shorter chain is refused, and nothing is written.
    grow(&world, &cast, &mut growing, &owner, "member-2").await;
    let mut late = revision(1, b"late");
    late["chain_seq"] = json!(1);
    let (status, refused) = world
        .call(Method::PUT, &path, Some(&cast.member_a), Some(late))
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{refused}");
    assert_eq!(refused["error_code"], "stale_chain");
    assert_eq!(read_seq().await, 1);
    let mut current = revision(1, b"current");
    current["chain_seq"] = json!(2);
    let (status, revised) = world
        .call(Method::PUT, &path, Some(&cast.member_a), Some(current))
        .await;
    assert_eq!(status, StatusCode::OK, "{revised}");
    assert_eq!(read_seq().await, 2);
}
