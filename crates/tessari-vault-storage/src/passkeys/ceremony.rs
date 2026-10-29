//! The ceremonies in flight: a challenge kept under its digest for five minutes, consumed by its first answer.

use sha2::{Digest, Sha256};
use tessari_vault_constants::{SESSION_TOKEN_BYTES, WEBAUTHN_CHALLENGE_TTL_SECS};
use tessari_vault_core::webauthn;
use tessari_vault_infrastructure::random;
use tessari_vault_infrastructure::tessaridb::Value;
use tessari_vault_types::base64url;

use super::PasskeyService;
use crate::access::bind;
use crate::enrolments::record::field;
use crate::sessions::hex;
use crate::{Error, Result};

fn key_of(challenge: &[u8]) -> String {
    hex(&Sha256::digest(challenge))
}

impl PasskeyService {
    /// Begin a ceremony for `purpose` (and `account`, for a registration): the challenge, base64url.
    pub(super) async fn begin(&self, purpose: &str, account: &str) -> Result<String> {
        let challenge = random::bytes::<SESSION_TOKEN_BYTES>()?;
        let lifetime = i64::try_from(WEBAUTHN_CHALLENGE_TTL_SECS).unwrap_or(i64::MAX);
        self.database
            .run_at(
                &self.place,
                "SET passkey_ceremonies:$key = { purpose: $purpose, account: $account } EXPIRE $lifetime;",
                vec![
                    bind("key", key_of(&challenge).as_str()),
                    bind("purpose", purpose),
                    bind("account", account),
                    (
                        "lifetime".to_owned(),
                        Value::Duration {
                            seconds: lifetime,
                            nanos: 0,
                        },
                    ),
                ],
            )
            .await?;
        Ok(base64url::encode(&challenge))
    }

    /// The ceremony the client data's challenge names, removed: `(purpose, account)`.
    pub(super) async fn take(
        &self,
        client_data_json: &[u8],
    ) -> Result<Option<(String, String, Vec<u8>)>> {
        let Some(challenge) = webauthn::challenge_of(client_data_json) else {
            return Ok(None);
        };
        let key = key_of(&challenge);
        let answers = self
            .database
            .read_at(
                &self.place,
                "GET passkey_ceremonies:$key;",
                vec![bind("key", key.as_str())],
            )
            .await?;
        let held = answers.into_iter().next();
        let (Some(purpose), Some(account)) =
            (field(held.clone(), "purpose"), field(held, "account"))
        else {
            return Ok(None);
        };
        let taken = self
            .database
            .transact_at(
                &self.place,
                "BEGIN; LET $held = GET passkey_ceremonies:$key; THROW 'not found' IF $held = NONE; \
                 DELETE passkey_ceremonies:$key; COMMIT;",
                "",
                vec![bind("key", key.as_str())],
            )
            .await
            .map_err(|error| crate::access::thrown(error.into()));
        match taken {
            Ok(_) => Ok(Some((purpose, account, challenge))),
            Err(Error::NotFound) => Ok(None),
            Err(error) => Err(error),
        }
    }
}
