//! Resetting a sign-in that rests on a retired service-key epoch: the platform admin alone may, because a sign-in
//! belongs to the account and not to one company, and a company's admin who could reset it would take over the
//! person's sign-in in every other company they belong to. The member's records stay sealed under their unlock
//! password throughout; the reset only lets them set a new sign-in password and authenticator.

use tessari_vault_core::access::Refusal;
use tessari_vault_infrastructure::tessaridb::{Answer, Number, Value};
use tessari_vault_types::AccountId;

use super::{EnrolmentService, EnrolmentToken};
use crate::access::{Actor, bind, thrown};
use crate::audit::{self, Event};
use crate::{Conflict, Error, Result};

impl EnrolmentService {
    /// Remove the credential of `account`, whose password rests on an epoch the key file no longer holds, on the
    /// trail of every company the account belongs to, and issue the link that enrols them again.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] for anyone but the platform admin, and for their own account; [`Error::NotFound`] for an
    /// account with no credential; [`Conflict::SignInCurrent`] when the key file still holds its epoch.
    pub async fn reset_retired(
        &self,
        actor: &Actor,
        account: &AccountId,
    ) -> Result<EnrolmentToken> {
        actor.platform()?;
        if actor.account() == account {
            return Err(Error::Refused(Refusal::Forbidden));
        }
        let answers = self
            .database
            .read_at(
                &self.place,
                "RETURN { epoch: (GET credentials:$a).key_epoch, \
                 companies: (SELECT company FROM memberships WHERE account = $a) };",
                vec![bind("a", account.as_str())],
            )
            .await?;
        let Some(Answer::Value {
            value: Value::Object(found),
            ..
        }) = answers.into_iter().next()
        else {
            return Err(unreadable("not one object"));
        };
        let epoch = match found.get("epoch") {
            Some(Value::Number(Number::Integer(epoch))) => {
                u32::try_from(*epoch).map_err(|_| unreadable("key_epoch"))?
            }
            _ => return Err(Error::NotFound),
        };
        if self.passwords.holds(epoch) {
            return Err(Error::Conflict(Conflict::SignInCurrent));
        }
        let companies: Vec<String> = match found.get("companies") {
            Some(Value::Array(rows)) => rows
                .iter()
                .filter_map(|row| match row {
                    Value::Object(fields) => match fields.get("company") {
                        Some(Value::String(company)) => Some(company.clone()),
                        _ => None,
                    },
                    _ => None,
                })
                .collect(),
            _ => return Err(unreadable("companies")),
        };
        let mut records = String::new();
        let mut parameters = vec![
            bind("account", account.as_str()),
            bind("epoch", i64::from(epoch)),
        ];
        for (n, company) in companies.iter().enumerate() {
            records.push_str(&audit::record_numbered(n));
            records.push(' ');
            parameters.extend(audit::binds_numbered(
                n,
                actor,
                &Event {
                    company,
                    action: "sign_in.reset",
                    target: account.as_str(),
                },
            ));
        }
        let script = format!(
            "BEGIN; THROW 'standing changed' IF (GET credentials:$account).key_epoch != $epoch; \
             DELETE credentials:$account; {records}COMMIT;"
        );
        self.database
            .transact_at(&self.place, &script, "", parameters)
            .await
            .map_err(|error| thrown(error.into()))?;
        self.issue(account, None).await
    }
}

fn unreadable(what: &str) -> Error {
    Error::Unreadable {
        record: "credential",
        reason: format!("{what} did not read back as written"),
    }
}
