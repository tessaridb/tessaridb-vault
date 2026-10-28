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
    /// The account already has a sign-in credential; a new one is recovery, which an enrolment link never is.
    #[error("the account is already enrolled")]
    Enrolled,
    /// The caller's standing changed between the decision and the write; nothing was written.
    #[error("the caller's standing changed; try again")]
    StandingChanged,
}

/// The result of a `tessari-vault-storage` call.
pub type Result<T> = std::result::Result<T, Error>;
