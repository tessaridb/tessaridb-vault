//! Every refusal the API answers with: a status and `{ error_code, message }`, the code stable and the message prose.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use tessari_vault_core::access::Refusal;
use tessari_vault_storage::{Conflict, Error};
use tessari_vault_types::PasswordError;

/// A refusal, ready to answer with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: &'static str,
}

#[derive(Serialize)]
struct Body {
    error_code: &'static str,
    message: &'static str,
}

impl ApiError {
    const fn new(status: StatusCode, code: &'static str, message: &'static str) -> Self {
        Self {
            status,
            code,
            message,
        }
    }

    /// No session, or one the store does not hold.
    pub const UNAUTHENTICATED: Self = Self::new(
        StatusCode::UNAUTHORIZED,
        "unauthenticated",
        "sign in to continue",
    );
    /// Not there, or not the caller's to see: one answer for both.
    pub const NOT_FOUND: Self = Self::new(StatusCode::NOT_FOUND, "not_found", "no such resource");
    /// Seen, and the caller's role does not include it.
    pub const FORBIDDEN: Self = Self::new(
        StatusCode::FORBIDDEN,
        "forbidden",
        "your role does not allow this",
    );
    /// The body or a parameter is not of the shape the route takes.
    pub const INVALID: Self = Self::new(
        StatusCode::BAD_REQUEST,
        "invalid_request",
        "the request is malformed",
    );
    /// Sign-in did not succeed; the answer is the same whatever the reason, so it says nothing about the account.
    pub const SIGN_IN_FAILED: Self = Self::new(
        StatusCode::UNAUTHORIZED,
        "sign_in_failed",
        "the address or the password is not right",
    );
    /// The one-time code is not the authenticator's current code.
    pub const WRONG_CODE: Self = Self::new(
        StatusCode::BAD_REQUEST,
        "wrong_code",
        "the code is not the authenticator's current code",
    );

    /// The passkey response was refused: not this ceremony's, not this relying party's, or not verified.
    pub const PASSKEY_REFUSED: Self = Self::new(
        StatusCode::BAD_REQUEST,
        "passkey_refused",
        "the passkey response was not accepted",
    );

    /// The new password breaks the policy named by `weak`.
    #[must_use]
    pub const fn weak_password(weak: PasswordError) -> Self {
        let message = match weak {
            PasswordError::Length => "a password is 8 to 256 characters",
            PasswordError::Control => "a password holds no control characters",
            PasswordError::Context => "a password does not contain the account's address",
        };
        Self::new(StatusCode::BAD_REQUEST, "weak_password", message)
    }

    /// Too many requests in this window.
    pub const THROTTLED: Self = Self::new(
        StatusCode::TOO_MANY_REQUESTS,
        "throttled",
        "too many requests; try again shortly",
    );
    /// The store did not answer, so nothing was decided.
    pub const UNAVAILABLE: Self = Self::new(
        StatusCode::SERVICE_UNAVAILABLE,
        "unavailable",
        "the service is unavailable; try again",
    );
    /// Something the server wrote did not read back.
    pub const INTERNAL: Self = Self::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "internal",
        "something went wrong",
    );

    /// The status it answers with.
    #[must_use]
    pub const fn status(&self) -> StatusCode {
        self.status
    }

    /// The stable code.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        self.code
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(Body {
                error_code: self.code,
                message: self.message,
            }),
        )
            .into_response()
    }
}

impl From<Error> for ApiError {
    fn from(error: Error) -> Self {
        match error {
            Error::Refused(Refusal::NotVisible) | Error::NotFound => Self::NOT_FOUND,
            Error::Refused(Refusal::Forbidden) => Self::FORBIDDEN,
            Error::Conflict(conflict) => conflicted(conflict),
            Error::Store(failure) => {
                // Indeterminate: the decision could not be made, which is never folded into a refusal.
                tracing::error!(error = %failure, "store unavailable; request refused as indeterminate");
                Self::UNAVAILABLE
            }
            Error::Unreadable { record, reason } => {
                tracing::error!(record, reason, "stored record unreadable");
                Self::INTERNAL
            }
        }
    }
}

const fn conflicted(conflict: Conflict) -> ApiError {
    let (code, message) = match conflict {
        Conflict::AlreadyMember => (
            "already_member",
            "the account is already a member of this company",
        ),
        Conflict::LastOwner => ("last_owner", "a company keeps at least one active owner"),
        Conflict::GroupNotEmpty => ("group_not_empty", "the group still holds other groups"),
        Conflict::WrongParent => (
            "wrong_parent",
            "a department sits in the company, a team in a department or the company",
        ),
        Conflict::NotAMember => (
            "not_a_member",
            "the account is not an active member of this company",
        ),
        Conflict::Enrolled => ("enrolled", "the account is already enrolled"),
        Conflict::PasskeyRegistered => ("passkey_registered", "the passkey is already registered"),
        Conflict::IdentityLinked => (
            "identity_linked",
            "the identity is already linked to an account",
        ),
        Conflict::StandingChanged => (
            "standing_changed",
            "your role changed while this ran; try again",
        ),
        Conflict::StaleRevision => (
            "stale_revision",
            "the record changed since it was read; read it again",
        ),
        Conflict::OutOfOrder => (
            "out_of_order",
            "a file part must start where the written bytes end, be a whole part unless it ends the file, and stay \
             within the file's size",
        ),
        Conflict::Incomplete => ("incomplete", "the file is not yet written to its size"),
        Conflict::KeysExist => ("keys_exist", "you keep a key set in this company already"),
        Conflict::KeyTaken => ("key_taken", "another member's key set holds that key id"),
        Conflict::StaleChain => (
            "stale_chain",
            "the statement is not the next one in the chain; read the chain again",
        ),
        Conflict::OutsideReach => (
            "outside_reach",
            "a wrap names a key set outside the group's reach, or one that holds a wrap already",
        ),
        Conflict::UnknownKey => (
            "unknown_key",
            "the key id is not the registered key of the one it names",
        ),
        Conflict::NoKey => ("no_key", "the group or member holds no key to share with"),
        Conflict::AlreadyShared => ("already_shared", "the record is shared with them already"),
        Conflict::RotationDue => (
            "rotation_due",
            "a share was removed; seal the next revision under a new record key",
        ),
        Conflict::StaleShares => (
            "stale_shares",
            "the revision must carry a wrap for exactly the record's author and shares; read it again",
        ),
    };
    ApiError::new(StatusCode::CONFLICT, code, message)
}
