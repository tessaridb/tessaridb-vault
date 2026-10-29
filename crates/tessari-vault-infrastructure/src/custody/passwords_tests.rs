use std::sync::Arc;
use std::time::{Duration, Instant};

use argon2::password_hash::{PasswordHash, PasswordVerifier};
use argon2::{Algorithm, Argon2, Params, Version};
use tessari_vault_constants::{
    PASSWORD_HASH_ITERATIONS_FLOOR, PASSWORD_HASH_MEMORY_KIB_FLOOR, PASSWORD_HASH_PARALLELISM_FLOOR,
};

use super::super::KeyRing;
use super::super::service_key::tests::file;
use super::{DUMMY_PASSWORD, PasswordCost, Passwords, Verdict, Verifier};
use crate::Error;

const PASSWORD: &str = "correct horse battery staple";

fn floor() -> PasswordCost {
    PasswordCost::new(
        PASSWORD_HASH_MEMORY_KIB_FLOOR,
        PASSWORD_HASH_ITERATIONS_FLOOR,
        PASSWORD_HASH_PARALLELISM_FLOOR,
    )
    .expect("the floor is a cost")
}

/// The key whose every byte is `byte`, as a key file spells it.
fn digits(byte: u8) -> String {
    format!("{byte:02x}").repeat(32)
}

/// A ring holding one `(epoch, byte)` key per entry, the last current.
fn ring(epochs: &[(u32, u8)]) -> KeyRing {
    let text: String = epochs
        .iter()
        .map(|(epoch, byte)| format!("{epoch} {}\n", digits(*byte)))
        .collect();
    KeyRing::load(&file(&text, 0o600)).expect("a key file")
}

fn passwords_at(key: u8, cost: PasswordCost, concurrency: usize) -> Arc<Passwords> {
    let ring = KeyRing::load(&file(&digits(key), 0o600)).expect("a key file");
    Passwords::new(&ring, cost, concurrency).expect("prepared")
}

/// What a credential row holds for `verifier`.
fn held(verifier: &Verifier) -> Option<(u32, &str)> {
    Some((verifier.epoch, verifier.phc.as_str()))
}

fn passwords(key: u8) -> Arc<Passwords> {
    passwords_at(key, floor(), 4)
}

#[tokio::test(flavor = "multi_thread")]
async fn a_password_verifies_against_its_own_hash_and_no_other_does() {
    let passwords = passwords(1);
    let stored = passwords.hash(PASSWORD).await.expect("hashed");
    assert!(
        stored.phc.starts_with("$argon2id$v=19$m=65536,t=3,p=1$"),
        "{}",
        stored.phc
    );
    assert!(!stored.phc.contains(PASSWORD));
    assert_eq!(stored.epoch, 1);
    assert_ne!(
        stored.phc,
        passwords.hash(PASSWORD).await.expect("hashed").phc,
        "a fresh salt each time"
    );
    assert_eq!(
        passwords.verify(PASSWORD, held(&stored)).await.ok(),
        Some(Verdict::Match { rehash: false })
    );
    for other in [
        "correct horse battery stapl",
        "Correct horse battery staple",
        "",
    ] {
        assert_eq!(
            passwords.verify(other, held(&stored)).await.ok(),
            Some(Verdict::Mismatch),
            "{other:?}"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_hash_is_keyed_by_the_pepper() {
    let stored = passwords(1).hash(PASSWORD).await.expect("hashed");
    assert_eq!(
        passwords(2).verify(PASSWORD, held(&stored)).await.ok(),
        Some(Verdict::Mismatch),
        "another service key, so another pepper"
    );
    let parsed = PasswordHash::new(&stored.phc).expect("a PHC string");
    let plain = Argon2::new(
        Algorithm::Argon2id,
        Version::V0x13,
        Params::try_from(&parsed).expect("its parameters"),
    );
    assert!(
        plain.verify_password(PASSWORD.as_bytes(), &parsed).is_err(),
        "the stored hash is not Argon2id of the bare password"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn the_same_characters_in_another_normal_form_are_the_same_password() {
    let passwords = passwords(1);
    let composed = "caf\u{e9} au lait, bien s\u{fb}r";
    let decomposed = "cafe\u{301} au lait, bien su\u{302}r";
    let stored = passwords.hash(composed).await.expect("hashed");
    assert_eq!(
        passwords.verify(decomposed, held(&stored)).await.ok(),
        Some(Verdict::Match { rehash: false })
    );
}

/// The quickest of three runs of `check`.
async fn quickest<F, Fut>(check: F) -> Duration
where
    F: Fn() -> Fut,
    Fut: Future<Output = ()>,
{
    let mut best = Duration::MAX;
    for _ in 0..3 {
        let started = Instant::now();
        check().await;
        best = best.min(started.elapsed());
    }
    best
}

#[tokio::test(flavor = "multi_thread")]
async fn an_unknown_account_costs_a_full_hash_and_never_matches() {
    let passwords = passwords(1);
    let stored = passwords.hash(PASSWORD).await.expect("hashed");
    assert_eq!(
        passwords.verify(PASSWORD, None).await.ok(),
        Some(Verdict::Mismatch)
    );
    assert_eq!(
        passwords.verify(DUMMY_PASSWORD, None).await.ok(),
        Some(Verdict::Mismatch),
        "not even the dummy's own password"
    );
    let known = quickest(|| async {
        let _ = passwords.verify("a wrong guess", held(&stored)).await;
    })
    .await;
    let unknown = quickest(|| async {
        let _ = passwords.verify("a wrong guess", None).await;
    })
    .await;
    assert!(
        unknown.saturating_mul(2) >= known,
        "unknown {unknown:?} against known {known:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_hash_made_below_the_current_cost_asks_to_be_made_again() {
    let stored = passwords(1).hash(PASSWORD).await.expect("hashed");
    let raised = PasswordCost::new(PASSWORD_HASH_MEMORY_KIB_FLOOR, 4, 1).expect("a cost");
    let passwords = passwords_at(1, raised, 4);
    assert_eq!(
        passwords.verify(PASSWORD, held(&stored)).await.ok(),
        Some(Verdict::Match { rehash: true })
    );
    assert_eq!(
        passwords.verify("a wrong guess", held(&stored)).await.ok(),
        Some(Verdict::Mismatch),
        "no rehash on a mismatch"
    );
    let fresh = passwords.hash(PASSWORD).await.expect("hashed");
    assert_eq!(
        passwords.verify(PASSWORD, held(&fresh)).await.ok(),
        Some(Verdict::Match { rehash: false })
    );
}

#[test]
fn a_cost_below_the_floor_is_refused() {
    for (memory, iterations, parallelism) in [
        (PASSWORD_HASH_MEMORY_KIB_FLOOR - 1, 3, 1),
        (PASSWORD_HASH_MEMORY_KIB_FLOOR, 2, 1),
        (PASSWORD_HASH_MEMORY_KIB_FLOOR, 3, 0),
    ] {
        assert!(
            matches!(
                PasswordCost::new(memory, iterations, parallelism),
                Err(Error::InvalidConfig {
                    field: "password_hash_cost",
                    ..
                })
            ),
            "{memory} {iterations} {parallelism}"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_stored_value_that_is_not_an_argon2id_hash_is_refused() {
    let passwords = passwords(1);
    for stored in [
        "not a hash",
        "$argon2i$v=19$m=65536,t=3,p=1$c2FsdHNhbHRzYWx0$aGFzaGhhc2hoYXNoaGFzaGhhc2hoYXNoaGFzaGhhc2g",
        "$2b$12$abcdefghijklmnopqrstuuJ0rD6d0b6Kq3QeJ1n9eQ8u1H5ZqXQyG",
    ] {
        assert!(
            matches!(
                passwords.verify(PASSWORD, Some((1, stored))).await,
                Err(Error::Custody { .. })
            ),
            "{stored}"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn hashes_beyond_the_concurrency_bound_wait_their_turn() {
    let single = passwords_at(1, floor(), 1);
    let alone = quickest(|| async {
        let _ = single.hash(PASSWORD).await;
    })
    .await;
    let started = Instant::now();
    let (first, second, third) = tokio::join!(
        single.hash(PASSWORD),
        single.hash(PASSWORD),
        single.hash(PASSWORD)
    );
    let together = started.elapsed();
    assert!(first.is_ok() && second.is_ok() && third.is_ok());
    assert!(
        together.saturating_mul(2) >= alone.saturating_mul(5),
        "three at one-at-a-time took {together:?}; one alone {alone:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_verifier_is_checked_under_its_epochs_pepper_and_asks_to_be_made_under_the_current() {
    let older = Passwords::new(&ring(&[(1, 1)]), floor(), 4).expect("prepared");
    let stored = older.hash(PASSWORD).await.expect("hashed");
    let passwords = Passwords::new(&ring(&[(1, 1), (2, 2)]), floor(), 4).expect("prepared");
    assert_eq!(
        passwords.verify(PASSWORD, held(&stored)).await.ok(),
        Some(Verdict::Match { rehash: true }),
        "a verifier made under epoch 1 still opens, and asks to move"
    );
    assert_eq!(
        passwords
            .verify(PASSWORD, Some((2, stored.phc.as_str())))
            .await
            .ok(),
        Some(Verdict::Mismatch),
        "checked under the pepper its row names, never another"
    );
    let fresh = passwords.hash(PASSWORD).await.expect("hashed");
    assert_eq!(fresh.epoch, 2);
    assert_eq!(
        passwords.verify(PASSWORD, held(&fresh)).await.ok(),
        Some(Verdict::Match { rehash: false })
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_verifier_of_an_epoch_the_ring_no_longer_holds_never_matches_and_says_nothing_more() {
    let stored = Passwords::new(&ring(&[(1, 1)]), floor(), 4)
        .expect("prepared")
        .hash(PASSWORD)
        .await
        .expect("hashed");
    let passwords = Passwords::new(&ring(&[(2, 2)]), floor(), 4).expect("prepared");
    assert_eq!(
        passwords.verify(PASSWORD, held(&stored)).await.ok(),
        Some(Verdict::Mismatch),
        "the answer an unknown account gets"
    );
}
