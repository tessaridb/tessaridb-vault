//! What the two sign-in steps answer, and the credential row they read.

use tessari_vault_infrastructure::tessaridb::{FromRecord, MappingFault, Row};

use crate::sessions::SessionToken;

/// A sign-in challenge as the browser holds it between the two steps; never printed.
pub struct Challenge(String);

impl std::fmt::Debug for Challenge {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Challenge(<redacted>)")
    }
}

impl Challenge {
    pub(super) const fn new(text: String) -> Self {
        Self(text)
    }

    /// The challenge's text, to hand to the browser once.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

/// How the password step ended.
#[derive(Debug)]
pub enum PasswordStep {
    /// The password is right; the code comes next.
    Challenge(Challenge),
    /// Anything else, one answer for all of it.
    Failed,
    /// Too many attempts for this address, pair or network address; try again later.
    Throttled,
}

/// How the code step ended.
#[derive(Debug)]
pub enum CodeStep {
    /// Signed in.
    Signed(SessionToken),
    /// No live challenge, or the account can no longer sign in.
    Unknown,
    /// Not the code of a step not yet used.
    WrongCode,
}

/// One `credentials` row as sign-in reads it.
pub(super) struct CredentialEntity {
    pub(super) verifier: String,
    pub(super) totp: String,
    pub(super) factor: String,
    pub(super) last_step: i64,
}

impl FromRecord for CredentialEntity {
    fn from_row(mut row: Row) -> std::result::Result<Self, MappingFault> {
        Ok(Self {
            verifier: row.take("verifier")?,
            totp: row.take("totp")?,
            factor: row.take("totp_factor")?,
            last_step: row.take("totp_last_step")?,
        })
    }
}
