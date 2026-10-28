//! A stub OpenID Connect issuer on a loopback port: discovery, a token endpoint that checks PKCE and spends each code
//! once, and a key set, signing ES256 ID tokens with whatever claims the test granted for the code.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::{Form, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use ring::digest::{SHA256, digest};
use ring::rand::SystemRandom;
use ring::signature::{ECDSA_P256_SHA256_FIXED_SIGNING, EcdsaKeyPair, KeyPair};
use serde::Deserialize;
use serde_json::{Value, json};

/// The client id the vault is registered under at the stub.
pub const CLIENT_ID: &str = "vault";
/// Where the stub sends the browser back to.
pub const REDIRECT: &str = "http://127.0.0.1:47999/sign-in/return";

/// What the ID token issued for one code says.
#[derive(Clone)]
pub struct Grant {
    pub challenge: String,
    pub subject: String,
    pub nonce: String,
    pub issuer: Option<String>,
    pub audience: Option<String>,
}

struct Shared {
    issuer: String,
    key: EcdsaKeyPair,
    grants: Mutex<HashMap<String, Grant>>,
}

/// The running stub.
#[derive(Clone)]
pub struct Issuer {
    shared: Arc<Shared>,
}

/// `bytes` in unpadded base64url.
pub fn b64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let sextet = |bits: u32| char::from(ALPHABET[usize::try_from(bits & 0x3f).expect("six bits")]);
    let (mut text, mut buffer, mut held) = (String::new(), 0_u32, 0_u32);
    for byte in bytes {
        buffer = ((buffer << 8) | u32::from(*byte)) & 0xffff;
        held = held.saturating_add(8);
        while held >= 6 {
            held = held.saturating_sub(6);
            text.push(sextet(buffer >> held));
        }
    }
    if held > 0 {
        text.push(sextet(buffer << 6_u32.saturating_sub(held)));
    }
    text
}

/// The S256 challenge of `verifier`.
pub fn challenge_of(verifier: &str) -> String {
    b64(digest(&SHA256, verifier.as_bytes()).as_ref())
}

#[derive(Deserialize)]
struct TokenRequest {
    grant_type: String,
    code: String,
    code_verifier: String,
    client_id: String,
}

async fn token(State(issuer): State<Issuer>, Form(request): Form<TokenRequest>) -> Response {
    let grant = issuer
        .shared
        .grants
        .lock()
        .expect("the grants")
        .remove(&request.code);
    let Some(grant) = grant.filter(|grant| {
        request.grant_type == "authorization_code"
            && request.client_id == CLIENT_ID
            && challenge_of(&request.code_verifier) == grant.challenge
    }) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "invalid_grant" })),
        )
            .into_response();
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("after 1970")
        .as_secs();
    let claims = json!({
        "iss": grant.issuer.unwrap_or_else(|| issuer.shared.issuer.clone()),
        "sub": grant.subject,
        "aud": grant.audience.unwrap_or_else(|| CLIENT_ID.to_owned()),
        "nonce": grant.nonce,
        "iat": now,
        "exp": now.saturating_add(300),
    });
    let input = format!(
        "{}.{}",
        b64(br#"{"alg":"ES256","typ":"JWT","kid":"stub"}"#),
        b64(claims.to_string().as_bytes())
    );
    let signature = issuer
        .shared
        .key
        .sign(&SystemRandom::new(), input.as_bytes())
        .expect("signed");
    let id_token = format!("{input}.{}", b64(signature.as_ref()));
    Json(json!({ "access_token": "unused", "token_type": "Bearer", "id_token": id_token }))
        .into_response()
}

impl Issuer {
    /// A stub serving on a free loopback port.
    pub async fn start() -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("a port");
        let issuer = format!("http://{}", listener.local_addr().expect("an address"));
        let random = SystemRandom::new();
        let pkcs8 =
            EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &random).expect("a key");
        let key =
            EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, pkcs8.as_ref(), &random)
                .expect("a key pair");
        let stub = Self {
            shared: Arc::new(Shared {
                issuer,
                key,
                grants: Mutex::new(HashMap::new()),
            }),
        };
        let point = stub.shared.key.public_key().as_ref().to_vec();
        let base = stub.shared.issuer.clone();
        let discovery = json!({
            "issuer": base,
            // The browser is never sent there in these tests, and a client sends users only to an https page.
            "authorization_endpoint": "https://id.stub.test/authorize",
            "token_endpoint": format!("{base}/token"),
            "jwks_uri": format!("{base}/jwks"),
        });
        let keys = json!({ "keys": [{
            "kty": "EC", "crv": "P-256", "kid": "stub", "use": "sig", "alg": "ES256",
            "x": b64(&point[1..33]), "y": b64(&point[33..65]),
        }] });
        let app = Router::new()
            .route(
                "/.well-known/openid-configuration",
                get(move || async move { Json(discovery) }),
            )
            .route("/jwks", get(move || async move { Json::<Value>(keys) }))
            .route("/token", post(token))
            .with_state(stub.clone());
        tokio::spawn(async move { axum::serve(listener, app).await });
        stub
    }

    /// The issuer identifier.
    pub fn issuer(&self) -> &str {
        &self.shared.issuer
    }

    /// Issue `code` with `grant`.
    pub fn grant(&self, code: &str, grant: Grant) {
        self.shared
            .grants
            .lock()
            .expect("the grants")
            .insert(code.to_owned(), grant);
    }
}
