//! A real TessariDB node, a fresh namespace on it, and the router over it, with the people the matrix needs.

use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU16, AtomicU32, Ordering};
use std::time::{Duration, Instant};

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Method, Request, StatusCode};
use serde_json::{Value, json};
use tessari_vault_api::{AppState, router};
use tessari_vault_infrastructure::config::PasskeyParty;
use tessari_vault_infrastructure::custody::{
    PasswordCost, Passwords, ServiceKey, SigninKeys, TotpSeal, VaultKeys,
};
use tessari_vault_infrastructure::oidc::OidcProvider;
use tessari_vault_infrastructure::tessaridb::{Database, Place, StoreConfig};
use tessari_vault_storage::{Custody, Storage, migrate};
use tessari_vault_types::{AccountId, Email, Name};
use tower::ServiceExt;

use crate::issuer::{CLIENT_ID, REDIRECT};

/// The relying party and origin the live server's passkeys are scoped to.
pub(crate) const PASSKEY_RP_ID: &str = "vault.test";
pub(crate) const PASSKEY_ORIGIN: &str = "https://vault.test";

pub(crate) const STORE_USER: &str = "vault_test";
pub(crate) const STORE_PASSWORD: &str = "a-test-password-for-the-live-suite";
/// The password of the server's own account, an editor of its namespace.
pub(crate) const SERVER_PASSWORD: &str = "the-server-account-of-the-live-suite";

/// What a deployment's store owner runs once: the server's namespace, and an account that may edit it and nothing else.
pub(crate) async fn declare_server_account(owner: &Database, namespace: &str, user: &str) {
    owner
        .run_unplaced(
            &format!(
                "DEFINE NAMESPACE {namespace}; USE NAMESPACE {namespace}; \
                 DEFINE USER {user} ON NAMESPACE {namespace} ROLE editor PASSWORD '{SERVER_PASSWORD}';"
            ),
            Vec::new(),
        )
        .await
        .expect("the owner declares the server's namespace and account");
}

static TURN: AtomicU32 = AtomicU32::new(0);
/// The next record id a test's browser chooses; every store is fresh, so a count is unique enough.
static RECORD: AtomicU32 = AtomicU32::new(1);

/// A record id as a browser chooses one: 32 lowercase hexadecimal characters.
pub fn fresh_record_id() -> String {
    format!("{:032x}", RECORD.fetch_add(1, Ordering::Relaxed))
}

/// The first port this run hands a node, below the range the system gives out by itself.
const PORT_BASE: u16 = 41_000;

/// Ports come from one counter for the whole run: two tests that each asked the system for a free port, then let it
/// go for their node to take, could be given the same one, and the world whose node lost would talk to the other's.
static PORT: AtomicU16 = AtomicU16::new(PORT_BASE);

/// The next port of this run's band that nothing on the host holds.
pub(crate) fn free_port() -> String {
    loop {
        let port = PORT.fetch_add(1, Ordering::Relaxed);
        assert!(
            (PORT_BASE..49_152).contains(&port),
            "the run used its port band"
        );
        if std::net::TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return format!("127.0.0.1:{port}");
        }
    }
}

/// A node process, stopped when dropped.
pub struct Node {
    child: Child,
    pub wire: String,
    pub http: String,
}

impl Drop for Node {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Node {
    /// A node closed by an initial user, on two free ports, holding its store in memory.
    pub async fn start() -> Self {
        Self::start_in(None).await
    }

    /// The same, holding its store in `data` when given.
    pub async fn start_in(data: Option<&std::path::Path>) -> Self {
        let binary = std::env::var("TESSARIDB_BIN")
            .expect("TESSARIDB_BIN names the engine binary the live suite runs against");
        // A port the system says is free can still refuse the node's own bind, so a node that exits before it
        // listens is started again on two other ports.
        for _ in 0..5 {
            let http = free_port();
            let wire = free_port();
            let mut command = Command::new(&binary);
            if let Some(data) = data {
                command.arg(data);
            }
            let child = command
                .args(["--http", &http, "--serve", &wire])
                .env("TESSARIDB_INITIAL_USER", STORE_USER)
                .env("TESSARIDB_INITIAL_PASSWORD", STORE_PASSWORD)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("the engine binary starts");
            let mut node = Self { child, wire, http };
            let started = Instant::now();
            loop {
                // Something answering on the port is not proof it is this node: one that lost its port has exited.
                let exited = !matches!(node.child.try_wait(), Ok(None));
                if exited {
                    break;
                }
                if tokio::net::TcpStream::connect(&node.wire).await.is_ok() {
                    return node;
                }
                assert!(
                    started.elapsed() < Duration::from_secs(20),
                    "the node did not listen"
                );
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }
        panic!("no node started on five pairs of ports");
    }
}

/// The server over a fresh namespace, and the tokens of the people in it.
pub struct World {
    pub app: Router,
    pub storage: Storage,
    /// The store and the place the server keeps everything in, for the rows that act behind the server's back.
    pub database: Database,
    pub place: Place,
    /// The company vault keys the server derives, for the rows that open a vault behind its back.
    pub vault_keys: std::sync::Arc<VaultKeys>,
    pub node: Node,
}

impl World {
    pub async fn new() -> Self {
        Self::build(None, None).await
    }

    /// A world whose node keeps its store in `data`.
    pub async fn on_disk(data: &std::path::Path) -> Self {
        Self::build(None, Some(data)).await
    }

    /// A world whose server signs people in through the stub issuer at `issuer`.
    pub async fn with_issuer(issuer: &str) -> Self {
        Self::registered_at(issuer, CLIENT_ID, REDIRECT).await
    }

    /// A world whose server signs people in through `issuer` as the client `client_id` returning to `redirect`.
    pub async fn registered_at(issuer: &str, client_id: &str, redirect: &str) -> Self {
        let provider = OidcProvider::discover(issuer, client_id, redirect)
            .await
            .expect("the issuer is discovered");
        Self::build(Some(std::sync::Arc::new(provider)), None).await
    }

    async fn build(
        oidc: Option<std::sync::Arc<OidcProvider>>,
        data: Option<&std::path::Path>,
    ) -> Self {
        let node = Node::start_in(data).await;
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
        // The server signs in as a deployment's does: an editor of its one namespace, declared by the store's owner,
        // never the owner itself. `database` stays the owner's, for the rows that act behind the server's back.
        let namespace = format!("vault_t{turn}");
        let server_user = format!("server_t{turn}");
        declare_server_account(&database, &namespace, &server_user).await;
        let server_database = Database::new(
            StoreConfig {
                address: node.wire.clone(),
                user: server_user,
                password: SERVER_PASSWORD.to_owned(),
                authority: None,
            },
            8,
        )
        .expect("a loopback pool");
        let key_file =
            std::env::temp_dir().join(format!("vault-live-key-{}-{turn}", std::process::id()));
        std::fs::write(&key_file, "ab".repeat(32)).expect("the key file");
        std::fs::set_permissions(
            &key_file,
            std::os::unix::fs::PermissionsExt::from_mode(0o600),
        )
        .expect("its mode");
        let service_key = ServiceKey::load(&key_file).expect("a service key");
        let passwords = Passwords::new(
            &service_key,
            PasswordCost::new(65_536, 3, 1).expect("the floor"),
            4,
        )
        .expect("the verifier");
        let seal = std::sync::Arc::new(TotpSeal::new(&service_key).expect("the seal"));
        let place = Place::new(namespace, "main").expect("plain names");
        migrate(&server_database, &place)
            .await
            .expect("the schema applies");
        migrate(&server_database, &place)
            .await
            .expect("the schema applies a second time unchanged");
        let signin_keys = SigninKeys::new(&service_key).expect("the sign-in keys");
        let vault_keys = std::sync::Arc::new(VaultKeys::new(&service_key).expect("the vault keys"));
        let storage = Storage::new(
            &server_database,
            &place,
            Custody {
                passwords,
                seal,
                signin_keys,
                vault_keys: std::sync::Arc::clone(&vault_keys),
            },
        );
        Self {
            app: router(AppState {
                storage: storage.clone(),
                oidc,
                passkeys: Some(PasskeyParty {
                    rp_id: PASSKEY_RP_ID.to_owned(),
                    origin: PASSKEY_ORIGIN.to_owned(),
                }),
            }),
            storage,
            database,
            place,
            vault_keys,
            node,
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
        let headers: Vec<(&str, String)> = token
            .map(|token| ("authorization", format!("Bearer {token}")))
            .into_iter()
            .collect();
        let (status, _, value) = self.call_with(method, path, &headers, body).await;
        (status, value)
    }

    /// Call the router with `headers`, answering the response headers too.
    pub async fn call_with(
        &self,
        method: Method,
        path: &str,
        headers: &[(&str, String)],
        body: Option<Value>,
    ) -> (StatusCode, HeaderMap, Value) {
        send(self.app.clone(), method, path, headers, body).await
    }

    /// Raw octets to `path`, as the browser uploads one part of a file.
    pub async fn put_octets(&self, path: &str, token: &str, bytes: Vec<u8>) -> (StatusCode, Value) {
        let request = Request::builder()
            .method(Method::PUT)
            .uri(path)
            .header("authorization", format!("Bearer {token}"))
            .header("content-type", "application/octet-stream")
            .body(Body::from(bytes))
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
    /// A record member A wrote.
    pub record_a: String,
    /// A small file member A wrote on that record.
    pub file_a: String,
    /// The key ids owner A, admin A and member A registered in company A.
    pub owner_a_key: String,
    pub admin_a_key: String,
    pub member_a_key: String,
}

pub(crate) async fn company(
    world: &World,
    platform: &str,
    name: &str,
    owner: &str,
) -> (String, String) {
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

/// A record's sealed parts as member A's browser sends them; the bytes mean nothing here.
pub fn sealed_record() -> Value {
    sealed_record_by(&"22".repeat(16))
}

/// Call `app` as [`World::call`] does, for a task that owns its own copy of the router.
pub async fn call_app(
    app: Router,
    method: Method,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let headers: Vec<(&str, String)> = token
        .map(|token| ("authorization", format!("Bearer {token}")))
        .into_iter()
        .collect();
    let (status, _, value) = send(app, method, path, &headers, body).await;
    (status, value)
}

async fn send(
    app: Router,
    method: Method,
    path: &str,
    headers: &[(&str, String)],
    body: Option<Value>,
) -> (StatusCode, HeaderMap, Value) {
    let mut request = Request::builder().method(method).uri(path);
    for (name, value) in headers {
        request = request.header(*name, value);
    }
    let request = match body {
        Some(body) => request
            .header("content-type", "application/json")
            .body(Body::from(body.to_string())),
        None => request.body(Body::empty()),
    }
    .expect("a request");
    let response = app.oneshot(request).await.expect("an answer");
    let status = response.status();
    let answered = response.headers().clone();
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .expect("a body");
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).expect("a JSON body")
    };
    (status, answered, value)
}

/// The same, signed under the registered key `author_key`.
pub fn sealed_record_by(author_key: &str) -> Value {
    use tessari_vault_types::base64url::encode;
    json!({
        "id": fresh_record_id(),
        "record_type": "login",
        "key_id": "11".repeat(16),
        "body": encode(b"sealed"),
        "signature": crate::records::signature(7),
        "author_key": author_key,
        "wrap": encode(&[9; 82]),
    })
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
        let (owner_a_key, admin_a_key, member_a_key) =
            ("44".repeat(16), "55".repeat(16), "22".repeat(16));
        for (token, key, seed) in [
            (&owner_a, &owner_a_key, 4),
            (&admin_a, &admin_a_key, 5),
            (&member_a, &member_a_key, 2),
        ] {
            let (status, body) = world
                .call(
                    Method::PUT,
                    &format!("/api/companies/{company_a}/keys/me"),
                    Some(token),
                    Some(crate::keys::key_set(key, seed)),
                )
                .await;
            assert_eq!(status, StatusCode::CREATED, "{body}");
        }
        let (status, record) = world
            .call(
                Method::POST,
                &format!("/api/companies/{company_a}/records"),
                Some(&member_a),
                Some(sealed_record()),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED, "{record}");
        let record_a = record["id"].as_str().expect("an id").to_owned();
        let files = format!("/api/companies/{company_a}/records/{record_a}/files");
        let (status, file) = world
            .call(
                Method::POST,
                &files,
                Some(&member_a),
                Some(json!({ "size": 6 })),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED, "{file}");
        let file_a = file["id"].as_str().expect("an id").to_owned();
        let (status, written) = world
            .put_octets(
                &format!("{files}/{file_a}?offset=0"),
                &member_a,
                b"sealed".to_vec(),
            )
            .await;
        assert_eq!(status, StatusCode::NO_CONTENT, "{written}");
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
            record_a,
            file_a,
            owner_a_key,
            admin_a_key,
            member_a_key,
        }
    }
}
