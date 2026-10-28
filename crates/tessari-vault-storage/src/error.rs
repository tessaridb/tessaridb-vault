//! The errors of `tessari-vault-storage`.

use tessari_vault_core::access::Refusal;

/// Why a storage call did not do what was asked.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The caller may not do this; the decision came from the one evaluator.
    #[error("refused: {0:?}")]
    Refused(Refusal),
    /// The request is understood and permitted, and the store's state does not allow it.
    #[error("{0}")]
    Conflict(Conflict),
    /// The thing named does not exist inside a company the caller may see.
    #[error("not found")]
    NotFound,
    /// The store could not be reached or refused for a reason no caller can fix.
    #[error(transparent)]
    Store(#[from] tessari_vault_infrastructure::Error),
    /// A stored record did not read back as the shape this version writes.
    #[error("{record} unreadable: {reason}")]
    Unreadable {
        /// Which kind of record.
        record: &'static str,
        /// What did not fit.
        reason: String,
    },
}

/// A state the store holds that the request would break.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Conflict {
    /// The account is already a member of the company.
    #[error("the account is already a member of this company")]
    AlreadyMember,
    /// The change would leave the company with no active owner.
    #[error("a company keeps at least one active owner")]
    LastOwner,
    /// The group still has departments or teams inside it.
    #[error("the group still holds other groups")]
    GroupNotEmpty,
    /// A group was asked to sit inside a group it cannot sit inside.
    #[error("a department sits in the company, a team in a department or the company")]
    WrongParent,
    /// The account is not an active member of the company, so it cannot join one of its groups.
    #[error("the account is not an active member of this company")]
    NotAMember,
    /// The passkey is registered already.
    #[error("the passkey is already registered")]
    PasskeyRegistered,
    /// The identity at that issuer is already linked to an account.
    #[error("the identity is already linked to an account")]
    IdentityLinked,
    /// The account already has a sign-in credential; a new one is recovery, which an enrolment link never is.
    #[error("the account is already enrolled")]
    Enrolled,
    /// The caller's standing changed between the decision and the write; nothing was written.
    #[error("the caller's standing changed; try again")]
    StandingChanged,
    /// The record has a newer revision than the one the caller read; nothing was written.
    #[error("the record changed since it was read")]
    StaleRevision,
    /// A file part did not start where the written bytes end, was not a whole part short of the end, or ran past
    /// the file's size; nothing was written.
    #[error("a file part out of order")]
    OutOfOrder,
    /// The file is not yet written to its size.
    #[error("the file is not yet complete")]
    Incomplete,
    /// The member keeps a key set in this company already.
    #[error("the member keeps a key set already")]
    KeysExist,
    /// Another member's key set holds that key id.
    #[error("the key id is taken")]
    KeyTaken,
    /// The statement is not the next one in the chain; nothing was written.
    #[error("the statement is not the next one in the chain")]
    StaleChain,
    /// The key id is not the caller's registered key in the company.
    #[error("the key is not the caller's registered key")]
    UnknownKey,
}

/// The result of a `tessari-vault-storage` call.
pub type Result<T> = std::result::Result<T, Error>;
