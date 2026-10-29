//! The `tessaridb-vault` binary itself, started as an operator would start it: the first person gets in through the
//! operator command, enrols and signs in over HTTP, the issuer's sign-in is wired, and SIGTERM stops it cleanly.
//!
//! Needs `TESSARIDB_BIN` and `TESSARIDB_VAULT_BIN` (the built `tessaridb-vault`).

use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use reqwest::StatusCode;
use reqwest::header::{LOCATION, SET_COOKIE};
use serde_json::{Value, json};
use tessari_vault_core::totp;
use tessari_vault_infrastructure::tessaridb::{Database, StoreConfig};

use crate::enrolment::base32;
use crate::issuer::{CLIENT_ID, Issuer, REDIRECT};
use crate::world::{
    Node, SERVER_PASSWORD, STORE_PASSWORD, STORE_USER, declare_server_account, free_port,
};

const ADMIN: &str = "ops@platform.test";
/// The binary's account on the node: an editor of the default namespace, `vault`.
const SERVER_USER: &str = "vault";
const PASSWORD: &str = "a passphrase of my own";

/// Everything the binary is started with: the node, the key file, the listen address and the issuer.
struct Setup {
    store: String,
    key_file: std::path::PathBuf,
    listen: String,
    issuer: Option<String>,
}

impl Setup {
    fn command(&self, binary: &str) -> Command {
        let mut command = Command::new(binary);
        command
            .env_clear()
            .env("TESSARIDB_VAULT_STORE", &self.store)
            .env("TESSARIDB_VAULT_STORE_USER", SERVER_USER)
            .env("TESSARIDB_VAULT_STORE_PASSWORD", SERVER_PASSWORD)
            .env("TESSARIDB_VAULT_SERVICE_KEY_FILE", &self.key_file)
            .env("TESSARIDB_VAULT_LISTEN", &self.listen)
            .env("TESSARIDB_VAULT_PLATFORM_ADMIN_EMAIL", ADMIN)
            .env("TESSARIDB_VAULT_PLATFORM_ADMIN_NAME", "Platform Ops");
        if let Some(issuer) = &self.issuer {
            command
                .env("TESSARIDB_VAULT_OIDC_ISSUER", issuer)
                .env("TESSARIDB_VAULT_OIDC_CLIENT_ID", CLIENT_ID)
                .env("TESSARIDB_VAULT_OIDC_REDIRECT", REDIRECT);
        }
        command
    }
}

fn binary() -> String {
    std::env::var("TESSARIDB_VAULT_BIN")
        .expect("TESSARIDB_VAULT_BIN names the built tessaridb-vault")
}

fn code(secret: &str, step: u64) -> String {
    format!("{:06}", totp::code_at(&base32(secret), step))
}

fn step() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("after 1970")
        .as_secs()
        / 30
}

async fn post(http: &reqwest::Client, url: String, body: Value) -> (StatusCode, Value) {
    let answer = http.post(url).json(&body).send().await.expect("an answer");
    let status = answer.status();
    (status, answer.json().await.unwrap_or(Value::Null))
}

/// Wait until `child` exits, at most `limit`.
fn exited(child: &mut Child, limit: Duration) -> Option<std::process::ExitStatus> {
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait().expect("a status") {
            return Some(status);
        }
        if started.elapsed() > limit {
            return None;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN and TESSARIDB_VAULT_BIN"]
async fn the_binary_lets_the_first_person_in_and_stops_on_sigterm() {
    let node = Node::start().await;
    let owner = Database::new(
        StoreConfig {
            address: node.wire.clone(),
            user: STORE_USER.to_owned(),
            password: STORE_PASSWORD.to_owned(),
            authority: None,
        },
        1,
    )
    .expect("a loopback pool");
    declare_server_account(&owner, "vault", SERVER_USER).await;
    let issuer = Issuer::start().await;
    let key_file = std::env::temp_dir().join(format!("vault-binary-key-{}", std::process::id()));
    std::fs::write(&key_file, "cd".repeat(32)).expect("the key file");
    std::fs::set_permissions(
        &key_file,
        std::os::unix::fs::PermissionsExt::from_mode(0o600),
    )
    .expect("its mode");
    let setup = Setup {
        store: node.wire.clone(),
        key_file,
        listen: free_port(),
        issuer: Some(issuer.issuer().to_owned()),
    };
    let Output { status, stdout, .. } = setup
        .command(&binary())
        .arg("enrolment-link")
        .output()
        .expect("the operator command runs");
    assert!(status.success(), "the operator command");
    let printed = String::from_utf8(stdout).expect("text");
    // The token is the line after its announcement; the process logs a line of its own as it stops.
    let mut lines = printed.lines();
    lines.find(|line| line.starts_with("enrolment token for "));
    let token = lines.next().expect("the token line").trim().to_owned();
    let mut server = setup
        .command(&binary())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("the server starts");
    let base = format!("http://{}", setup.listen);
    let http = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("a client");
    let started = Instant::now();
    while tokio::net::TcpStream::connect(&setup.listen).await.is_err() {
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "the server did not listen"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let (status, begun) = post(
        &http,
        format!("{base}/api/enrolment/begin"),
        json!({ "token": token }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{begun}");
    let secret = begun["secret"].as_str().expect("a secret").to_owned();
    let enrolled_at = step();
    let complete =
        json!({ "token": token, "password": PASSWORD, "code": code(&secret, enrolled_at) });
    let (status, body) = post(&http, format!("{base}/api/enrolment/complete"), complete).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let (status, body) = post(
        &http,
        format!("{base}/api/sign-in"),
        json!({ "email": ADMIN, "password": PASSWORD }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let submitted = json!({ "challenge": body["challenge"], "code": code(&secret, enrolled_at.saturating_add(1)) });
    let (status, body) = post(&http, format!("{base}/api/sign-in/code"), submitted).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let session = body["session"].as_str().expect("a session");
    let me = http
        .get(format!("{base}/api/me"))
        .bearer_auth(session)
        .send()
        .await
        .expect("an answer");
    assert_eq!(me.status(), StatusCode::OK);
    let me: Value = me.json().await.expect("JSON");
    assert_eq!(
        (
            me["account"]["email"].as_str(),
            me["platform_admin"].as_bool()
        ),
        (Some(ADMIN), Some(true)),
        "{me}"
    );
    let start = http
        .get(format!("{base}/api/sign-in/oidc"))
        .send()
        .await
        .expect("an answer");
    assert_eq!(start.status(), StatusCode::SEE_OTHER);
    let location = start.headers()[LOCATION].to_str().expect("an address");
    assert!(
        location.starts_with("https://id.stub.test/authorize?"),
        "{location}"
    );
    assert!(
        start.headers()[SET_COOKIE]
            .to_str()
            .expect("a cookie")
            .starts_with("__Host-vault-oidc=")
    );
    // No public address is configured, so there is no passkey relying party and nothing to begin.
    let passkey = http
        .post(format!("{base}/api/sign-in/passkey/begin"))
        .send()
        .await
        .expect("an answer");
    assert_eq!(passkey.status(), StatusCode::NOT_FOUND);
    let pid = server.id().to_string();
    assert!(
        Command::new("kill")
            .args(["-TERM", &pid])
            .status()
            .expect("kill runs")
            .success()
    );
    let stopped = exited(&mut server, Duration::from_secs(10)).expect("it stops within 10 s");
    assert!(stopped.success(), "a clean stop: {stopped}");
    // A configured issuer that cannot be discovered refuses the start rather than serving without it.
    let refused = Setup {
        issuer: Some("http://127.0.0.1:1".to_owned()),
        listen: free_port(),
        ..setup
    };
    let mut refusing = refused
        .command(&binary())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("starts");
    let status = exited(&mut refusing, Duration::from_secs(20)).expect("it refuses within 20 s");
    assert!(
        !status.success(),
        "an undiscoverable issuer refuses the start"
    );
    let _ = std::fs::remove_file(&refused.key_file);
}
