//! A real TessariDB node, a fresh namespace on it, and the router over it, with the people the matrix needs.

use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use serde_json::{Value, json};
use tessari_vault_api::{AppState, router};
use tessari_vault_infrastructure::tessaridb::{Database, Place, StoreConfig};
use tessari_vault_storage::{Storage, migrate};
use tessari_vault_types::{AccountId, Email, Name};
use tower::ServiceExt;

const STORE_USER: &str = "vault_test";
const STORE_PASSWORD: &str = "a-test-password-for-the-live-suite";

static TURN: AtomicU32 = AtomicU32::new(0);

/// A node process, stopped when dropped.
pub struct Node {
    child: Child,
    pub wire: String,
}

impl Drop for Node {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Node {
    /// A node closed by an initial user, on two free ports.
    pub async fn start() -> Self {
        let binary = std::env::var("TESSARIDB_BIN")
            .expect("TESSARIDB_BIN names the engine binary the live suite runs against");
        let http_probe = std::net::TcpListener::bind("127.0.0.1:0").expect("a free port");
        let wire_probe = std::net::TcpListener::bind("127.0.0.1:0").expect("a second free port");
        let http = format!(
            "127.0.0.1:{}",
            http_probe.local_addr().expect("bound").port()
        );
        let wire = format!(
            "127.0.0.1:{}",
            wire_probe.local_addr().expect("bound").port()
        );
        drop((http_probe, wire_probe));
        let child = Command::new(&binary)
            .args(["--http", &http, "--serve", &wire])
            .env("TESSARIDB_INITIAL_USER", STORE_USER)
            .env("TESSARIDB_INITIAL_PASSWORD", STORE_PASSWORD)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("the engine binary starts");
        let node = Self { child, wire };
        let started = Instant::now();
        while tokio::net::TcpStream::connect(&node.wire).await.is_err() {
            assert!(
                started.elapsed() < Duration::from_secs(20),
                "the node did not listen"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        node
    }
}

/// The server over a fresh namespace, and the tokens of the people in it.
pub struct World {
    pub app: Router,
    pub storage: Storage,
    _node: Node,
}

impl World {
    pub async fn new() -> Self {
        let node = Node::start().await;
        let database = Database::new(
            StoreConfig {
                address: node.wire.clone(),
                user: STORE_USER.to_owned(),
                password: STORE_PASSWORD.to_owned(),
                authority: None,
            },
            8,
        )
        .expect("a loopback pool");
        let turn = TURN.fetch_add(1, Ordering::Relaxed);
        let place = Place::new(format!("vault_t{turn}"), "main").expect("plain names");
        migrate(&database, &place)
            .await
            .expect("the schema applies");
        migrate(&database, &place)
            .await
            .expect("the schema applies a second time unchanged");
        let storage = Storage::new(&database, &place);
        Self {
            app: router(AppState {
                storage: storage.clone(),
            }),
            storage,
            _node: node,
        }
    }

    /// A session for `account`, as sign-in will issue one.
    pub async fn token(&self, account: &str) -> String {
        let id = AccountId::parse(account).expect("an account id");
        self.storage
            .sessions
            .issue(&id)
            .await
            .expect("a session")
            .expose()
            .to_owned()
    }

    /// The platform admin's token.
    pub async fn platform_admin(&self) -> String {
        let id = self
            .storage
            .accounts
            .system_ensure_platform_admin(
                &Email::parse("ops@platform.test").expect("address"),
                &Name::parse("Platform Ops").expect("name"),
            )
            .await
            .expect("the platform admin");
        self.token(id.as_str()).await
    }

    /// Call the router directly, with no interface in the loop.
    pub async fn call(
        &self,
        method: Method,
        path: &str,
        token: Option<&str>,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let mut request = Request::builder().method(method).uri(path);
        if let Some(token) = token {
            request = request.header("authorization", format!("Bearer {token}"));
        }
        let request = match body {
            Some(body) => request
                .header("content-type", "application/json")
                .body(Body::from(body.to_string())),
            None => request.body(Body::empty()),
        }
        .expect("a request");
        let response = self.app.clone().oneshot(request).await.expect("an answer");
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
            .await
            .expect("a body");
        let value = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).expect("a JSON body")
        };
        (status, value)
    }
}

/// Two companies with their people and groups, built through the API itself.
pub struct Cast {
    pub platform: String,
    pub company_a: String,
    pub company_b: String,
    pub owner_a: String,
    pub admin_a: String,
    pub member_a: String,
    pub admin_b: String,
    pub admin_b_id: String,
    pub owner_a_id: String,
    pub admin_a_id: String,
    pub member_a_id: String,
    pub department_a: String,
    pub team_a: String,
}

async fn company(world: &World, platform: &str, name: &str, owner: &str) -> (String, String) {
    let (status, body) = world
        .call(
            Method::POST,
            "/api/companies",
            Some(platform),
            Some(json!({ "name": name, "owner_email": owner, "owner_name": "Owner" })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    (
        body["id"].as_str().expect("an id").to_owned(),
        body["owner_account_id"]
            .as_str()
            .expect("the owner")
            .to_owned(),
    )
}

pub async fn add_member(world: &World, company: &str, by: &str, email: &str, role: &str) -> String {
    let (status, body) = world
        .call(
            Method::POST,
            &format!("/api/companies/{company}/members"),
            Some(by),
            Some(json!({ "email": email, "name": "Someone", "role": role })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["account_id"].as_str().expect("an id").to_owned()
}

impl Cast {
    pub async fn new(world: &World) -> Self {
        let platform = world.platform_admin().await;
        let (company_a, owner_a_id) = company(world, &platform, "Alpha", "owner@alpha.test").await;
        let (company_b, owner_b_id) = company(world, &platform, "Beta", "owner@beta.test").await;
        let owner_a = world.token(&owner_a_id).await;
        let owner_b = world.token(&owner_b_id).await;
        let admin_a_id = add_member(world, &company_a, &owner_a, "admin@alpha.test", "admin").await;
        let member_a_id =
            add_member(world, &company_a, &owner_a, "member@alpha.test", "member").await;
        let admin_b_id = add_member(world, &company_b, &owner_b, "admin@beta.test", "admin").await;
        let admin_a = world.token(&admin_a_id).await;
        let member_a = world.token(&member_a_id).await;
        let admin_b = world.token(&admin_b_id).await;
        let (status, department) = world
            .call(
                Method::POST,
                &format!("/api/companies/{company_a}/groups"),
                Some(&admin_a),
                Some(json!({ "kind": "department", "name": "Engineering" })),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED, "{department}");
        let department_a = department["id"].as_str().expect("an id").to_owned();
        let (status, team) = world
            .call(
                Method::POST,
                &format!("/api/companies/{company_a}/groups"),
                Some(&admin_a),
                Some(json!({ "kind": "team", "name": "Platform", "parent": department_a })),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED, "{team}");
        let team_a = team["id"].as_str().expect("an id").to_owned();
        Self {
            platform,
            company_a,
            company_b,
            owner_a,
            admin_a,
            member_a,
            admin_b,
            admin_b_id,
            owner_a_id,
            admin_a_id,
            member_a_id,
            department_a,
            team_a,
        }
    }
}
