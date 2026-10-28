//! Every refusal the API answers with: a status and `{ error_code, message }`, the code stable and the message prose.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use tessari_vault_core::access::Refusal;
use tessari_vault_storage::{Conflict, Error};

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
        Conflict::StandingChanged => (
            "standing_changed",
            "your role changed while this ran; try again",
        ),
    };
    ApiError::new(StatusCode::CONFLICT, code, message)
}
