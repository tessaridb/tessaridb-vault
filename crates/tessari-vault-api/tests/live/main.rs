//! The live suite: the server's router over a real TessariDB node, called directly with no interface in the loop.
//!
//! Run with `TESSARIDB_BIN=<engine binary> TESSARIDB_VAULT_BIN=target/debug/tessaridb-vault cargo test -p
//! tessari-vault-api --test live -- --ignored --skip oauth_e2e` after `cargo build -p tessari-vault-node`;
//! `oauth_e2e` also needs a running TessariDB OAuth server and runs on its own (see that module).

#![cfg(test)]

mod audit;
mod audit_reads;
mod authenticator;
mod binary;
mod canaries;
mod enrolment;
mod files;
mod group_keys;
mod issuer;
mod keyed_vault;
mod keys;
mod oauth_e2e;
mod oidc;
mod page;
mod passkeys;
mod record_types;
mod records;
mod recovery;
mod revocation;
mod rules;
mod shares;
mod sign_in;
mod world;

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};
use tessari_vault_api::routes;
use world::{Cast, World};

/// Routes that refuse nobody by design, each with the reason; every other route has a probe.
const EXEMPT: [(Method, &str, &str); 6] = [
    (
        Method::GET,
        "/api/sign-in/methods",
        "which ways in this server offers: names no account",
    ),
    (
        Method::POST,
        "/api/sign-in/passkey/begin",
        "a fresh challenge that names no account",
    ),
    (
        Method::GET,
        "/",
        "the page: the same committed bytes for anyone, no data",
    ),
    (Method::GET, "/app.js", "the page's script: no data"),
    (Method::GET, "/app.css", "the page's style: no data"),
    (Method::GET, "/favicon.svg", "the product mark: no data"),
];

/// Who a probe is sent as.
#[derive(Clone, Copy, Debug)]
enum Who {
    Owner,
    Member,
    OtherAdmin,
}

/// One route's negative rows: what a request with no session or an unknown one gets, what the low-privilege identity
/// gets, and what another company's admin gets.
struct Probe {
    path: String,
    body: Option<Value>,
    unsigned: StatusCode,
    low: Option<(Who, StatusCode)>,
    cross: Option<(Who, StatusCode)>,
}

fn probe(cast: &Cast, method: &Method, pattern: &str) -> Option<Probe> {
    let a = &cast.company_a;
    let path = pattern
        .replace("{company}", a)
        .replace("{group}", &cast.team_a)
        .replace("{account}", &cast.member_a_id)
        .replace("{record}", &cast.record_a)
        .replace("{file}", &cast.file_a)
        .replace("{target}", &cast.owner_a_id)
        .replace("{request}", &"e9".repeat(16));
    let manage = Some((Who::Member, StatusCode::FORBIDDEN));
    let hidden = Some((Who::OtherAdmin, StatusCode::NOT_FOUND));
    let not_author = Some((Who::Owner, StatusCode::NOT_FOUND));
    let platform = Some((Who::Owner, StatusCode::FORBIDDEN));
    let other_platform = Some((Who::OtherAdmin, StatusCode::FORBIDDEN));
    let row = |body: Option<Value>, low, cross| {
        Some(Probe {
            path: path.clone(),
            body,
            unsigned: StatusCode::UNAUTHORIZED,
            low,
            cross,
        })
    };
    // The unsigned routes: the token in the body is the credential, so a token that is not one is the refusal.
    let by_token = |body: Value, unsigned: StatusCode| {
        Some(Probe {
            path: path.clone(),
            body: Some(body),
            unsigned,
            low: None,
            cross: None,
        })
    };
    match (method.as_str(), pattern) {
        // Self routes: nothing lower than signed in; the cross-company row is the listing test below.
        ("GET", "/api/me" | "/api/companies") => row(None, None, None),
        ("POST", "/api/companies") => row(
            Some(json!({ "name": "Gamma", "owner_email": "o@gamma.test", "owner_name": "O" })),
            platform,
            other_platform,
        ),
        ("POST", "/api/companies/{company}/suspend" | "/api/companies/{company}/resume") => {
            row(None, platform, other_platform)
        }
        ("GET", "/api/companies/{company}/members" | "/api/companies/{company}/groups") => {
            row(None, None, hidden)
        }
        ("GET", "/api/companies/{company}/groups/{group}/members") => row(None, None, hidden),
        ("GET", "/api/companies/{company}/audit" | "/api/companies/{company}/audit/verify") => {
            row(None, manage, hidden)
        }
        ("POST", "/api/companies/{company}/members") => row(
            Some(json!({ "email": "new@alpha.test", "name": "New", "role": "member" })),
            manage,
            hidden,
        ),
        ("PATCH", "/api/companies/{company}/members/{account}") => {
            row(Some(json!({ "role": "admin" })), manage, hidden)
        }
        ("DELETE", "/api/companies/{company}/members/{account}") => row(None, manage, hidden),
        ("POST", "/api/companies/{company}/members/{account}/enrolment") => {
            row(None, manage, hidden)
        }
        ("POST", "/api/sign-in") => by_token(
            json!({ "email": "nobody@alpha.test", "password": "a long enough password" }),
            StatusCode::UNAUTHORIZED,
        ),
        ("POST", "/api/sign-in/code") => by_token(
            json!({ "challenge": "cd".repeat(32), "code": "000000" }),
            StatusCode::UNAUTHORIZED,
        ),
        ("DELETE", "/api/session") => row(None, None, None),
        // No issuer is configured in this world, so the two unsigned issuer routes answer that there is none.
        ("GET", "/api/sign-in/oidc" | "/api/sign-in/oidc/return") => Some(Probe {
            path: format!("{path}?state=cd&code=ef"),
            body: None,
            unsigned: StatusCode::NOT_FOUND,
            low: None,
            cross: None,
        }),
        ("POST", "/api/me/identities/oidc") => row(None, None, None),
        ("POST", "/api/me/passkeys/begin") => row(None, None, None),
        ("POST", "/api/me/passkeys") => row(
            Some(json!({ "client_data_json": "e30", "attestation_object": "oA" })),
            None,
            None,
        ),
        ("POST", "/api/sign-in/passkey") => by_token(
            json!({ "credential_id": "AAAA", "client_data_json": "e30", "authenticator_data": "AA",
                    "signature": "AA", "user_handle": "AA" }),
            StatusCode::UNAUTHORIZED,
        ),
        ("POST", "/api/me/identities/oidc/return") => {
            row(Some(json!({ "query": "state=cd&code=ef" })), None, None)
        }
        ("POST", "/api/enrolment/begin") => {
            by_token(json!({ "token": "cd".repeat(32) }), StatusCode::NOT_FOUND)
        }
        ("POST", "/api/enrolment/complete") => by_token(
            json!({ "token": "cd".repeat(32), "password": "a long enough password", "code": "000000" }),
            StatusCode::NOT_FOUND,
        ),
        ("POST", "/api/companies/{company}/groups") => row(
            Some(json!({ "kind": "department", "name": "Sales" })),
            manage,
            hidden,
        ),
        ("PATCH", "/api/companies/{company}/groups/{group}") => {
            row(Some(json!({ "name": "Renamed" })), manage, hidden)
        }
        ("DELETE", "/api/companies/{company}/groups/{group}") => row(None, manage, hidden),
        ("POST", "/api/companies/{company}/groups/{group}/members") => row(
            Some(json!({ "account_id": cast.member_a_id })),
            manage,
            hidden,
        ),
        ("DELETE", "/api/companies/{company}/groups/{group}/members/{account}") => {
            row(None, manage, hidden)
        }
        // Every member keeps records; the lower row is the owner, who is not the record's author.
        ("GET", "/api/companies/{company}/records") => row(None, None, hidden),
        ("POST", "/api/companies/{company}/records") => {
            row(Some(world::sealed_record()), None, hidden)
        }
        ("GET", "/api/companies/{company}/records/{record}") => row(None, not_author, hidden),
        ("GET", "/api/companies/{company}/record-types") => row(None, None, hidden),
        ("POST", "/api/companies/{company}/record-types") => row(
            Some(json!({ "allowed_roles": ["member"], "definition": cast.record_a })),
            manage,
            hidden,
        ),
        // Admins key a group and fill its wraps; every member reads a group's key and their own wrap of it.
        ("PUT", "/api/companies/{company}/groups/{group}/key") => row(
            Some(
                json!({ "key_id": "c0".repeat(16), "public": crate::records::encoded(&[0; 32]),
                         "wraps": [] }),
            ),
            manage,
            hidden,
        ),
        // Owners keep the recovery key; every member reads its public half, and an owner their own wrap of it.
        ("POST", "/api/companies/{company}/recovery-key") => row(
            Some(
                json!({ "key_id": "e1".repeat(16), "public": crate::records::encoded(&[0; 32]),
                         "wraps": [] }),
            ),
            manage,
            hidden,
        ),
        ("GET", "/api/companies/{company}/recovery-key") => row(None, None, hidden),
        ("GET", "/api/companies/{company}/recovery-key/mine") => row(None, manage, hidden),
        // A member escrows their own keys; only an owner reads an escrow; each member reads the notices about them.
        ("PUT", "/api/companies/{company}/keys/me/escrow") => row(
            Some(json!({ "recovery_key_id": "e1".repeat(16), "wrap": "AA" })),
            None,
            hidden,
        ),
        ("GET", "/api/companies/{company}/keys/{account}/escrow") => row(None, manage, hidden),
        ("GET", "/api/companies/{company}/notices/me") => row(None, None, hidden),
        // A member asks and redeems their own request; only an owner lists and hands one over.
        ("POST", "/api/companies/{company}/recovery-requests") => row(
            Some(json!({ "handoff_public": crate::records::encoded(&[0; 32]) })),
            None,
            hidden,
        ),
        ("GET", "/api/companies/{company}/recovery-requests") => row(None, manage, hidden),
        ("POST", "/api/companies/{company}/recovery-requests/{request}/handoff") => {
            row(Some(json!({ "wrap": "AA" })), manage, hidden)
        }
        ("POST", "/api/companies/{company}/recovery-requests/{request}/redeem") => {
            row(None, None, hidden)
        }
        ("POST", "/api/companies/{company}/keys/me/rewrap") => row(
            Some(json!({ "request": "e1".repeat(16), "wrapped": "AA",
                         "kdf": { "salt": crate::records::encoded(&[0; 16]), "memory_kib": 47_104,
                                  "passes": 2, "lanes": 1 } })),
            None,
            hidden,
        ),
        ("GET", "/api/companies/{company}/groups/{group}/key") => row(None, None, hidden),
        ("GET", "/api/companies/{company}/groups/{group}/key/pending") => row(None, manage, hidden),
        ("POST", "/api/companies/{company}/groups/{group}/key/wraps") => {
            row(Some(json!({ "wraps": [] })), manage, hidden)
        }
        // Every member keeps their own key set and reads the company's directory and chain.
        ("PUT", "/api/companies/{company}/keys/me") => row(
            Some(crate::keys::key_set(&"aa".repeat(16), 10)),
            None,
            hidden,
        ),
        ("GET", "/api/companies/{company}/keys/me" | "/api/companies/{company}/keys") => {
            row(None, None, hidden)
        }
        ("GET", "/api/companies/{company}/statements") => row(None, None, hidden),
        ("POST", "/api/companies/{company}/statements") => row(
            Some(
                json!({ "seq": 1, "bytes": "AA", "signature": crate::records::signature(0),
                         "owner_signing": crate::records::encoded(&[0; 32]) }),
            ),
            Some((Who::Member, StatusCode::FORBIDDEN)),
            hidden,
        ),
        // A file is reached only through its record's author, so the owner and another company find nothing.
        ("POST", "/api/companies/{company}/records/{record}/files") => {
            row(Some(json!({ "size": 10 })), not_author, hidden)
        }
        ("PUT", "/api/companies/{company}/records/{record}/files/{file}") => Some(Probe {
            path: format!("{path}?offset=0"),
            body: None,
            unsigned: StatusCode::UNAUTHORIZED,
            low: not_author,
            cross: hidden,
        }),
        ("GET", "/api/companies/{company}/records/{record}/files/{file}") => Some(Probe {
            path: format!("{path}?offset=0&limit=1"),
            body: None,
            unsigned: StatusCode::UNAUTHORIZED,
            low: not_author,
            cross: hidden,
        }),
        ("POST", "/api/companies/{company}/records/{record}/shares") => row(
            Some(
                json!({ "member": cast.owner_a_id, "right": "read", "key_id": "44".repeat(16),
                         "wrap": crate::records::encoded(&[1; 82]) }),
            ),
            not_author,
            hidden,
        ),
        ("DELETE", "/api/companies/{company}/records/{record}/shares/{target}") => {
            row(None, not_author, hidden)
        }
        ("PUT", "/api/companies/{company}/records/{record}") => {
            let mut body = world::sealed_record();
            body["revision"] = json!(1);
            body.as_object_mut()
                .map(|fields| fields.remove("record_type"));
            row(Some(body), not_author, hidden)
        }
        _ => None,
    }
}

fn token(cast: &Cast, who: Who) -> &str {
    match who {
        Who::Owner => &cast.owner_a,
        Who::Member => &cast.member_a,
        Who::OtherAdmin => &cast.admin_b,
    }
}

/// A refusal carries the error shape and nothing else; no record is leaked beside it.
fn refusal_only(body: &Value) -> bool {
    body.as_object().is_some_and(|fields| {
        fields.len() == 2 && fields.contains_key("error_code") && fields.contains_key("message")
    })
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn every_route_refuses_the_unsigned_the_lower_role_and_another_company() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let (mut source, mut enforced, mut verified) = (0_u32, 0_u32, 0_u32);
    let mut missing = Vec::new();
    let mut exempt = Vec::new();
    for route in routes() {
        source = source.saturating_add(1);
        if let Some((_, _, reason)) = EXEMPT
            .iter()
            .find(|(method, path, _)| route.method == *method && *path == route.path)
        {
            exempt.push(format!("{} {}: {reason}", route.method, route.path));
            continue;
        }
        let Some(probe) = probe(&cast, &route.method, route.path) else {
            missing.push(format!("{} {}", route.method, route.path));
            continue;
        };
        enforced = enforced.saturating_add(1);
        let (status, body) = world
            .call(route.method.clone(), &probe.path, None, probe.body.clone())
            .await;
        assert_eq!(
            status, probe.unsigned,
            "{} {} unsigned: {body}",
            route.method, route.path
        );
        assert!(refusal_only(&body), "{body}");
        let (status, body) = world
            .call(
                route.method.clone(),
                &probe.path,
                Some("ab".repeat(32).as_str()),
                probe.body.clone(),
            )
            .await;
        assert_eq!(
            status, probe.unsigned,
            "{} {} unknown token: {body}",
            route.method, route.path
        );
        for (who, expected) in [probe.low, probe.cross].into_iter().flatten() {
            let (status, body) = world
                .call(
                    route.method.clone(),
                    &probe.path,
                    Some(token(&cast, who)),
                    probe.body.clone(),
                )
                .await;
            assert_eq!(
                status, expected,
                "{} {} as {who:?}: {body}",
                route.method, route.path
            );
            assert!(
                refusal_only(&body),
                "{} {} as {who:?} leaked {body}",
                route.method,
                route.path
            );
        }
        verified = verified.saturating_add(1);
    }
    println!(
        "paths: source={source} | enforced={enforced} | verified={verified} | exempt={exempt:?}"
    );
    assert!(missing.is_empty(), "routes with no probe: {missing:?}");
    let open = u32::try_from(exempt.len()).expect("a few");
    assert_eq!(
        (enforced, verified),
        (source.saturating_sub(open), source.saturating_sub(open))
    );
    // The refused calls changed nothing: the member is still a member, the team still exists.
    let (status, body) = world
        .call(
            Method::GET,
            &format!("/api/companies/{}/members", cast.company_a),
            Some(&cast.owner_a),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["members"].as_array().map(Vec::len), Some(3), "{body}");
    let (status, body) = world
        .call(
            Method::GET,
            &format!("/api/companies/{}/groups", cast.company_a),
            Some(&cast.owner_a),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["groups"].as_array().map(Vec::len), Some(2), "{body}");
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn a_caller_lists_only_the_companies_they_belong_to() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let (status, body) = world
        .call(Method::GET, "/api/companies", Some(&cast.admin_b), None)
        .await;
    assert_eq!(status, StatusCode::OK);
    let ids: Vec<&str> = body["companies"]
        .as_array()
        .expect("a list")
        .iter()
        .filter_map(|c| c["id"].as_str())
        .collect();
    assert_eq!(ids, vec![cast.company_b.as_str()], "{body}");
    let (_, body) = world
        .call(Method::GET, "/api/companies", Some(&cast.platform), None)
        .await;
    assert_eq!(
        body["companies"].as_array().map(Vec::len),
        Some(2),
        "{body}"
    );
    let (status, body) = world
        .call(Method::GET, "/api/me", Some(&cast.member_a), None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["account"]["email"], "member@alpha.test");
    assert_eq!(body["platform_admin"], false);
}
