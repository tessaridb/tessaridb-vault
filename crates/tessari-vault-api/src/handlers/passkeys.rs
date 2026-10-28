//! Passkeys: a signed-in member registers one; anyone signs in with one. Without a configured public address there
//! is no relying party, and every route answers that there is none.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use tessari_vault_infrastructure::config::PasskeyParty;
use tessari_vault_storage::passkeys::Assertion;
use tessari_vault_types::base64url;

use crate::ApiError;
use crate::caller::{Body, Caller, Unsigned};
use crate::state::AppState;

fn party(state: &AppState) -> Result<&PasskeyParty, ApiError> {
    state.passkeys.as_ref().ok_or(ApiError::NOT_FOUND)
}

#[derive(Serialize)]
pub(crate) struct RegistrationOptions {
    challenge: String,
    rp_id: String,
    user_id: String,
    user_name: String,
    exclude: Vec<String>,
}

#[derive(Serialize)]
pub(crate) struct SignInOptions {
    challenge: String,
    rp_id: String,
}

#[derive(Deserialize)]
pub(crate) struct Registration {
    client_data_json: String,
    attestation_object: String,
}

#[derive(Serialize)]
pub(crate) struct Registered {
    credential_id: String,
}

#[derive(Deserialize)]
pub(crate) struct SignedAssertion {
    credential_id: String,
    client_data_json: String,
    authenticator_data: String,
    signature: String,
    user_handle: String,
}

#[derive(Serialize)]
pub(crate) struct Session {
    session: String,
}

/// What the browser needs to create a passkey for the caller: user verification is required.
pub(crate) async fn begin_registration(
    State(state): State<AppState>,
    Caller(actor): Caller,
) -> Result<Json<RegistrationOptions>, ApiError> {
    let party = party(&state)?;
    let account = state.storage.accounts.me(&actor).await?;
    let challenge = state
        .storage
        .passkeys
        .begin_registration(actor.account())
        .await?;
    let exclude = state
        .storage
        .passkeys
        .credentials_of(actor.account())
        .await?;
    Ok(Json(RegistrationOptions {
        challenge,
        rp_id: party.rp_id.clone(),
        user_id: base64url::encode(actor.account().as_str().as_bytes()),
        user_name: account.email.as_str().to_owned(),
        exclude,
    }))
}

/// Register the passkey the browser created for the caller.
pub(crate) async fn register(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Body(body): Body<Registration>,
) -> Result<(StatusCode, Json<Registered>), ApiError> {
    let party = party(&state)?;
    let credential_id = state
        .storage
        .passkeys
        .register(
            actor.account(),
            party,
            &body.client_data_json,
            &body.attestation_object,
        )
        .await?
        .ok_or(ApiError::PASSKEY_REFUSED)?;
    Ok((StatusCode::CREATED, Json(Registered { credential_id })))
}

/// What the browser needs to ask any passkey of this relying party: no account is named, so none is disclosed.
pub(crate) async fn begin_sign_in(
    State(state): State<AppState>,
    _: Unsigned,
) -> Result<Json<SignInOptions>, ApiError> {
    let party = party(&state)?;
    let challenge = state.storage.passkeys.begin_sign_in().await?;
    Ok(Json(SignInOptions {
        challenge,
        rp_id: party.rp_id.clone(),
    }))
}

/// A session for the account the passkey's assertion proves; every failure is one answer.
pub(crate) async fn sign_in(
    State(state): State<AppState>,
    _: Unsigned,
    Body(body): Body<SignedAssertion>,
) -> Result<(StatusCode, Json<Session>), ApiError> {
    let party = party(&state)?;
    let assertion = Assertion {
        credential_id: &body.credential_id,
        client_data_json: &body.client_data_json,
        authenticator_data: &body.authenticator_data,
        signature: &body.signature,
        user_handle: &body.user_handle,
    };
    let session = state
        .storage
        .passkeys
        .sign_in(party, &assertion)
        .await?
        .ok_or(ApiError::SIGN_IN_FAILED)?;
    Ok((
        StatusCode::CREATED,
        Json(Session {
            session: session.expose().to_owned(),
        }),
    ))
}
