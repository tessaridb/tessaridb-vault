//! Own sign-in: the password, then one code, then a session; and every way it says no.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};
use tessari_vault_core::totp;

use crate::enrolment::{base32, begin, complete, issue};
use crate::world::{Cast, World, add_member};

const PASSWORD: &str = "a passphrase of my own";

/// An enrolled member of company A: their authenticator's secret.
async fn enrolled(world: &World, cast: &Cast, email: &str) -> String {
    enrolled_using(world, cast, email).await.0
}

/// An enrolled member of company A: their authenticator's secret and the code the enrolment spent.
async fn enrolled_using(world: &World, cast: &Cast, email: &str) -> (String, String) {
    let account = add_member(world, &cast.company_a, &cast.admin_a, email, "member").await;
    let token = issue(world, &cast.admin_a, &cast.company_a, &account)
        .await
        .1["token"]
        .as_str()
        .expect("a token")
        .to_owned();
    let secret = begin(world, &token).await.1["secret"]
        .as_str()
        .expect("a secret")
        .to_owned();
    let spent = code(&secret, 0);
    let (status, body) = complete(world, &token, PASSWORD, &spent).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    (secret, spent)
}

/// The code of `secret` for the step `ahead` steps after the current one.
fn code(secret: &str, ahead: u64) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("after 1970")
        .as_secs();
    format!(
        "{:06}",
        totp::code_at(&base32(secret), (now / 30).saturating_add(ahead))
    )
}

async fn password(world: &World, email: &str, password: &str) -> (StatusCode, Value) {
    world
        .call(
            Method::POST,
            "/api/sign-in",
            None,
            Some(json!({ "email": email, "password": password })),
        )
        .await
}

async fn submit(world: &World, challenge: &str, code: &str) -> (StatusCode, Value) {
    world
        .call(
            Method::POST,
            "/api/sign-in/code",
            None,
            Some(json!({ "challenge": challenge, "code": code })),
        )
        .await
}

fn challenge(answer: &(StatusCode, Value)) -> String {
    assert_eq!(answer.0, StatusCode::OK, "{}", answer.1);
    answer.1["challenge"]
        .as_str()
        .expect("a challenge")
        .to_owned()
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn the_password_then_one_code_signs_in_and_signing_out_ends_the_session() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let (secret, spent) = enrolled_using(&world, &cast, "signer@alpha.test").await;
    let first = challenge(&password(&world, "Signer@Alpha.test ", PASSWORD).await);
    let (status, body) = submit(&world, &first, &spent).await;
    assert_eq!(
        (status, body["error_code"].as_str()),
        (StatusCode::BAD_REQUEST, Some("wrong_code")),
        "the enrolment already used this step"
    );
    // One code, computed once: recomputing it later could name the step after, which nobody has used.
    let next = code(&secret, 1);
    let (status, body) = submit(&world, &first, &next).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let session = body["session"].as_str().expect("a session").to_owned();
    assert_eq!(
        world
            .call(Method::GET, "/api/me", Some(&session), None)
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        submit(&world, &first, &next).await.0,
        StatusCode::UNAUTHORIZED,
        "a challenge is used once"
    );
    let second = challenge(&password(&world, "signer@alpha.test", PASSWORD).await);
    let (status, body) = submit(&world, &second, &next).await;
    assert_eq!(
        (status, body["error_code"].as_str()),
        (StatusCode::BAD_REQUEST, Some("wrong_code")),
        "a code signs in once"
    );
    let (status, _) = world
        .call(Method::DELETE, "/api/session", Some(&session), None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(
        world
            .call(Method::GET, "/api/me", Some(&session), None)
            .await
            .0,
        StatusCode::UNAUTHORIZED,
        "signed out"
    );
}

/// The quickest of three runs of `attempt`.
async fn quickest(world: &World, email: &str, guess: &str) -> (Duration, (StatusCode, Value)) {
    let mut best = Duration::MAX;
    let mut answer = None;
    for _ in 0..3 {
        let started = Instant::now();
        answer = Some(password(world, email, guess).await);
        best = best.min(started.elapsed());
    }
    (best, answer.expect("three answers"))
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn no_answer_or_timing_says_whether_an_address_has_an_account() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    enrolled(&world, &cast, "known@alpha.test").await;
    add_member(
        &world,
        &cast.company_a,
        &cast.admin_a,
        "unenrolled@alpha.test",
        "member",
    )
    .await;
    let (known, wrong) = quickest(&world, "known@alpha.test", "not the passphrase").await;
    assert_eq!(wrong.0, StatusCode::UNAUTHORIZED, "{}", wrong.1);
    for email in ["nobody@alpha.test", "unenrolled@alpha.test"] {
        let (took, answer) = quickest(&world, email, "not the passphrase").await;
        assert_eq!(answer, wrong, "{email}: the same status and body");
        assert!(
            took.saturating_mul(2) >= known,
            "{email} took {took:?}; a known address {known:?}"
        );
    }
    assert_eq!(password(&world, "not an address", PASSWORD).await, wrong);
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn repeated_tries_back_off_by_pair_and_by_address_without_locking_the_account() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let secret = enrolled(&world, &cast, "tried@alpha.test").await;
    for _ in 0..3 {
        assert_eq!(
            password(&world, "tried@alpha.test", "a breached guess")
                .await
                .0,
            StatusCode::UNAUTHORIZED
        );
    }
    assert_eq!(
        password(&world, "tried@alpha.test", "a breached guess")
            .await
            .0,
        StatusCode::TOO_MANY_REQUESTS,
        "the fourth try of one pair"
    );
    let first = challenge(&password(&world, "tried@alpha.test", PASSWORD).await);
    assert_eq!(
        submit(&world, &first, &code(&secret, 1)).await.0,
        StatusCode::CREATED,
        "the owner still gets in"
    );
    for turn in 0..5 {
        password(&world, "tried@alpha.test", &format!("guess number {turn}")).await;
    }
    assert_eq!(
        password(&world, "tried@alpha.test", PASSWORD).await.0,
        StatusCode::TOO_MANY_REQUESTS,
        "the eleventh try at one address in the window"
    );
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn five_wrong_codes_end_the_challenge() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let secret = enrolled(&world, &cast, "fumbler@alpha.test").await;
    let first = challenge(&password(&world, "fumbler@alpha.test", PASSWORD).await);
    let right = code(&secret, 1);
    let wrong = format!(
        "{:06}",
        right.parse::<u32>().expect("digits").wrapping_add(500_000) % 1_000_000
    );
    for _ in 0..5 {
        assert_eq!(
            submit(&world, &first, &wrong).await.0,
            StatusCode::BAD_REQUEST
        );
    }
    assert_eq!(
        submit(&world, &first, &right).await.0,
        StatusCode::UNAUTHORIZED,
        "the challenge ended"
    );
    let second = challenge(&password(&world, "fumbler@alpha.test", PASSWORD).await);
    assert_eq!(
        submit(&world, &second, &right).await.0,
        StatusCode::CREATED,
        "a new challenge works"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "needs TESSARIDB_BIN"]
async fn one_code_sent_on_two_challenges_at_once_signs_in_once() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    // Each round is its own account, so the pair counter of one does not refuse the next round's challenges.
    for round in 1..=3_u64 {
        let email = format!("racer{round}@alpha.test");
        let secret = enrolled(&world, &cast, &email).await;
        let first = challenge(&password(&world, &email, PASSWORD).await);
        let second = challenge(&password(&world, &email, PASSWORD).await);
        let code = code(&secret, 1);
        let (one, two) = tokio::join!(
            submit(&world, &first, &code),
            submit(&world, &second, &code)
        );
        let signed = [one.0, two.0]
            .iter()
            .filter(|status| **status == StatusCode::CREATED)
            .count();
        assert_eq!(signed, 1, "round {round}: {one:?} {two:?}");
    }
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn one_network_address_trying_many_accounts_backs_off() {
    let world = World::new().await;
    // Twenty addresses, each tried once, from one network address: no account and no pair is over its count.
    for turn in 0..20 {
        let (status, body) = password(
            &world,
            &format!("spray{turn}@alpha.test"),
            "a sprayed guess",
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "try {turn}: {body}");
    }
    assert_eq!(
        password(&world, "spray20@alpha.test", "a sprayed guess")
            .await
            .0,
        StatusCode::TOO_MANY_REQUESTS,
        "the twenty-first try from one network address"
    );
}
