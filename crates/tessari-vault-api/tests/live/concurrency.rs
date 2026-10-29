//! Requests on one company at the same time: every audited request appends to the company's one trail head, so
//! concurrent ones contend there, and the store reports the loser rather than waiting. A refused transaction wrote
//! nothing, so the server sends it again; none of these requests may answer 503.

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use crate::world::{Cast, World, sealed_record_by};

/// The answers to `calls`, all made at once, in the order the calls were given.
async fn together(
    world: &World,
    token: &str,
    calls: Vec<(Method, String, Option<Value>)>,
) -> Vec<(StatusCode, Value)> {
    let mut running = tokio::task::JoinSet::new();
    for (at, (method, path, body)) in calls.into_iter().enumerate() {
        let app = world.app.clone();
        let token = token.to_owned();
        running.spawn(async move {
            (
                at,
                crate::world::call_app(app, method, &path, Some(&token), body).await,
            )
        });
    }
    let mut answered = Vec::new();
    while let Some(done) = running.join_next().await {
        answered.push(done.expect("the call ran"));
    }
    answered.sort_by_key(|(at, _)| *at);
    answered.into_iter().map(|(_, answer)| answer).collect()
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn concurrent_requests_on_one_company_all_answer() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let (b, admin) = (&cast.company_b, &cast.admin_b);
    let key = "77".repeat(16);
    let (status, body) = world
        .call(
            Method::PUT,
            &format!("/api/companies/{b}/keys/me"),
            Some(admin),
            Some(crate::keys::key_set(&key, 7)),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let (status, made) = world
        .call(
            Method::POST,
            &format!("/api/companies/{b}/records"),
            Some(admin),
            Some(sealed_record_by(&key)),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{made}");
    let record = made["id"].as_str().expect("an id");

    let opens = (0..8)
        .map(|_| {
            (
                Method::GET,
                format!("/api/companies/{b}/records/{record}"),
                None,
            )
        })
        .collect();
    for (status, body) in together(&world, admin, opens).await {
        assert_eq!(status, StatusCode::OK, "a concurrent open: {body}");
    }

    let changes = (0..8)
        .map(|n| {
            let body = json!({ "kind": "department", "name": format!("Unit {n}") });
            (
                Method::POST,
                format!("/api/companies/{b}/groups"),
                Some(body),
            )
        })
        .collect();
    for (status, body) in together(&world, admin, changes).await {
        assert_eq!(status, StatusCode::CREATED, "a concurrent change: {body}");
    }
}
