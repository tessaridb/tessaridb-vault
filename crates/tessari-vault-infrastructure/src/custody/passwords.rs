//! Sign-in password hashing: Argon2id (RFC 9106) over an HMAC-SHA-256 of the password under the pepper derived from
//! the service key.
//!
//! The pepper lives outside the database, so a dump of the stored hashes alone cannot be attacked offline. There is one
//! pepper per epoch of the service key, and a verifier names the epoch it was made under; because the pepper is applied
//! before Argon2id, a verifier moves to a new epoch only when its password is presented again. The
//! password is put in Normalization Form C before anything else (NIST SP 800-63B-4 §3.1.1.2), so the same characters
//! typed on two keyboards are one password. Every hash and every check runs on the blocking pool, a bounded number at
//! a time: each one holds the configured memory, and the bound is what keeps a burst of sign-ins from holding more.

use std::fmt;
use std::sync::Arc;

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Algorithm, Argon2, Params, Version};
use ring::hmac;
use ring::rand::{SecureRandom, SystemRandom};
use tessari_vault_constants::{
    PASSWORD_HASH_ITERATIONS_FLOOR, PASSWORD_HASH_MEMORY_KIB_FLOOR, PASSWORD_HASH_OUTPUT_BYTES,
    PASSWORD_HASH_PARALLELISM_FLOOR, PASSWORD_SALT_BYTES,
};
use tokio::sync::Semaphore;
use unicode_normalization::UnicodeNormalization;
use zeroize::Zeroizing;

use super::KeyRing;
use crate::{Error, Result};

/// What the dummy hash is made of; nothing is ever compared against it as a real password.
const DUMMY_PASSWORD: &str = "no account has this password";

/// The Argon2id cost every new hash is made at; never below the floor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PasswordCost {
    memory_kib: u32,
    iterations: u32,
    parallelism: u32,
}

impl PasswordCost {
    /// A cost of `memory_kib` KiB, `iterations` passes and `parallelism` lanes.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidConfig`] for `password_hash_cost` when any of the three is below its floor.
    pub const fn new(memory_kib: u32, iterations: u32, parallelism: u32) -> Result<Self> {
        if memory_kib < PASSWORD_HASH_MEMORY_KIB_FLOOR
            || iterations < PASSWORD_HASH_ITERATIONS_FLOOR
            || parallelism < PASSWORD_HASH_PARALLELISM_FLOOR
        {
            return Err(Error::InvalidConfig {
                field: "password_hash_cost",
                reason: "below the floor of 65536 KiB, 3 iterations and 1 lane",
            });
        }
        Ok(Self {
            memory_kib,
            iterations,
            parallelism,
        })
    }

    /// Whether a hash made with `params` was made below this cost.
    fn exceeds(&self, params: &Params) -> bool {
        params.m_cost() < self.memory_kib
            || params.t_cost() < self.iterations
            || params.p_cost() < self.parallelism
    }
}

/// What a presented password proved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// It is the account's password; `rehash` asks for it to be stored again at the current cost.
    Match {
        /// The stored hash was made at a lower cost than the current one.
        rehash: bool,
    },
    /// It is not, or there is no account.
    Mismatch,
}

/// A stored verifier: the PHC string and the epoch whose pepper it was made under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verifier {
    /// The epoch of the service key whose pepper the hash was made under.
    pub epoch: u32,
    /// The Argon2id PHC string.
    pub phc: String,
}

/// The peppers and the cost; hashing and verifying run off the request workers, a bounded number at a time.
pub struct Passwords {
    /// The epoch a new verifier is made under.
    current: u32,
    /// One pepper per epoch of the ring.
    peppers: Vec<(u32, hmac::Key)>,
    cost: PasswordCost,
    random: SystemRandom,
    /// A hash at the current cost that an unknown account is checked against, so it costs what a known one does.
    dummy: String,
    gate: Semaphore,
}

impl fmt::Debug for Passwords {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Passwords")
            .field("cost", &self.cost)
            .finish_non_exhaustive()
    }
}

const fn custody(reason: &'static str) -> Error {
    Error::Custody { reason }
}

impl Passwords {
    /// Prepare hashing at `cost` under the peppers the epochs of `ring` derive, at most `concurrency` at a time.
    /// Called once at startup: it derives one key per epoch and makes one hash.
    ///
    /// # Errors
    ///
    /// [`Error::Custody`] when a pepper cannot be derived or the dummy hash cannot be made.
    pub fn new(ring: &KeyRing, cost: PasswordCost, concurrency: usize) -> Result<Arc<Self>> {
        let peppers = ring
            .epochs()
            .into_iter()
            .filter_map(|epoch| ring.epoch(epoch).map(|key| (epoch, key)))
            .map(|(epoch, key)| {
                key.pepper()
                    .map(|pepper| (epoch, hmac::Key::new(hmac::HMAC_SHA256, pepper.as_slice())))
            })
            .collect::<Result<Vec<_>>>()?;
        let mut passwords = Self {
            current: ring.current_epoch(),
            peppers,
            cost,
            random: SystemRandom::new(),
            dummy: String::new(),
            gate: Semaphore::new(concurrency.max(1)),
        };
        passwords.dummy = passwords.hash_now(DUMMY_PASSWORD)?;
        Ok(Arc::new(passwords))
    }

    /// `password` hashed for storage under the current epoch's pepper.
    ///
    /// # Errors
    ///
    /// [`Error::Custody`] when the random source or the hash fails.
    pub async fn hash(self: &Arc<Self>, password: &str) -> Result<Verifier> {
        let password = Zeroizing::new(password.to_owned());
        self.off_workers(move |passwords| {
            passwords.hash_now(&password).map(|phc| Verifier {
                epoch: passwords.current,
                phc,
            })
        })
        .await
    }

    /// Whether `password` matches `stored`, a verifier and the epoch its row names. `None` is an unknown account,
    /// and a verifier of an epoch the ring no longer holds is one whose password can no longer be checked: both are
    /// checked against the dummy hash so they cost the same, and are never a match.
    ///
    /// # Errors
    ///
    /// [`Error::Custody`] when `stored` is not an Argon2id PHC string or the hash fails.
    pub async fn verify(
        self: &Arc<Self>,
        password: &str,
        stored: Option<(u32, &str)>,
    ) -> Result<Verdict> {
        let password = Zeroizing::new(password.to_owned());
        let stored = stored.map(|(epoch, phc)| (epoch, phc.to_owned()));
        self.off_workers(move |passwords| {
            let held = stored.as_ref().and_then(|(epoch, phc)| {
                passwords.pepper(*epoch).map(|pepper| (*epoch, pepper, phc))
            });
            match held {
                Some((epoch, pepper, phc)) => {
                    passwords
                        .verify_now(pepper, &password, phc)
                        .map(|verdict| match verdict {
                            Verdict::Match { rehash } => Verdict::Match {
                                rehash: rehash || epoch != passwords.current,
                            },
                            Verdict::Mismatch => Verdict::Mismatch,
                        })
                }
                None => passwords
                    .verify_now(passwords.current_pepper()?, &password, &passwords.dummy)
                    .map(|_| Verdict::Mismatch),
            }
        })
        .await
    }

    /// Whether the ring still holds `epoch`, so a verifier made under it can be checked.
    #[must_use]
    pub fn holds(&self, epoch: u32) -> bool {
        self.pepper(epoch).is_some()
    }

    /// The pepper of `epoch`, when the ring holds it.
    fn pepper(&self, epoch: u32) -> Option<&hmac::Key> {
        self.peppers
            .iter()
            .find(|(held, _)| *held == epoch)
            .map(|(_, pepper)| pepper)
    }

    fn current_pepper(&self) -> Result<&hmac::Key> {
        self.pepper(self.current)
            .ok_or(custody("the ring holds no pepper for its current epoch"))
    }

    /// Run `work` on the blocking pool once a permit is free; the permit is held until it finishes.
    async fn off_workers<T, F>(self: &Arc<Self>, work: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&Self) -> Result<T> + Send + 'static,
    {
        let _permit = self
            .gate
            .acquire()
            .await
            .map_err(|_| custody("the password gate closed"))?;
        let passwords = Arc::clone(self);
        tokio::task::spawn_blocking(move || work(&passwords))
            .await
            .map_err(|_| custody("the password task did not finish"))?
    }

    /// `pepper`'s HMAC of `password` in Normalization Form C: what Argon2id is handed instead of the password.
    fn peppered(pepper: &hmac::Key, password: &str) -> Zeroizing<Vec<u8>> {
        let normalised = Zeroizing::new(password.nfc().collect::<String>());
        Zeroizing::new(hmac::sign(pepper, normalised.as_bytes()).as_ref().to_vec())
    }

    fn hash_now(&self, password: &str) -> Result<String> {
        let mut salt = [0_u8; PASSWORD_SALT_BYTES];
        self.random
            .fill(&mut salt)
            .map_err(|_| custody("the random source failed"))?;
        let salt = SaltString::encode_b64(&salt).map_err(|_| custody("the salt did not encode"))?;
        let params = Params::new(
            self.cost.memory_kib,
            self.cost.iterations,
            self.cost.parallelism,
            Some(PASSWORD_HASH_OUTPUT_BYTES),
        )
        .map_err(|_| custody("the hash parameters are not usable"))?;
        Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
            .hash_password(&Self::peppered(self.current_pepper()?, password), &salt)
            .map(|hash| hash.to_string())
            .map_err(|_| custody("hashing failed"))
    }

    fn verify_now(&self, pepper: &hmac::Key, password: &str, stored: &str) -> Result<Verdict> {
        let not_ours = || custody("a stored password hash that is not an Argon2id PHC string");
        let parsed = PasswordHash::new(stored).map_err(|_| not_ours())?;
        if Algorithm::try_from(parsed.algorithm) != Ok(Algorithm::Argon2id) {
            return Err(not_ours());
        }
        let params = Params::try_from(&parsed).map_err(|_| not_ours())?;
        let version = parsed
            .version
            .map_or(Ok(Version::V0x13), Version::try_from)
            .map_err(|_| not_ours())?;
        let rehash = version != Version::V0x13 || self.cost.exceeds(&params);
        // The library compares the derived tag with the stored one in constant time.
        match Argon2::new(Algorithm::Argon2id, version, params)
            .verify_password(&Self::peppered(pepper, password), &parsed)
        {
            Ok(()) => Ok(Verdict::Match { rehash }),
            Err(argon2::password_hash::Error::Password) => Ok(Verdict::Mismatch),
            Err(_) => Err(custody("verifying failed")),
        }
    }
}

#[cfg(test)]
#[path = "passwords_tests.rs"]
mod tests;
