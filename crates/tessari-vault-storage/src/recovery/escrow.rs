//! Escrow (C7): each member's browser wraps its own private keys to the company recovery key the owner vouched for.
//! Only an owner reads a member's escrow, and the read leaves that member a notice in the same transaction, so a
//! member can always see when their keys were reached.

use tessari_vault_core::access::Action;
use tessari_vault_types::{AccountId, CompanyId, KeyId};

use super::entity::{Notice, NoticeEntity};
use super::repository::Escrowed;
use super::service::RecoveryService;
use crate::access::{self, Actor, recorded};
use crate::{Conflict, Error, Result};

impl RecoveryService {
    /// Keep the caller's escrow `wrap` to the company's recovery key `recovery_key_id`, replacing an earlier one.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] per the decision; [`Error::NotFound`] when no owner has made a recovery key;
    /// [`Conflict::UnknownKey`] when `recovery_key_id` is not the standing one; [`Conflict::NoKey`] when the caller
    /// keeps no key set.
    pub async fn escrow(
        &self,
        actor: &Actor,
        company: &CompanyId,
        recovery_key_id: &KeyId,
        wrap: &[u8],
    ) -> Result<()> {
        let standing = access::authorize(
            &self.database,
            &self.place,
            actor,
            company,
            Action::KeepKeys,
        )
        .await?;
        let standing_key = self
            .repository
            .get(&self.place, company)
            .await?
            .ok_or(Error::NotFound)?;
        if standing_key.key_id != recovery_key_id.as_str() {
            return Err(Error::Conflict(Conflict::UnknownKey));
        }
        self.repository
            .keep_escrow(
                &self.place,
                recorded(
                    actor,
                    company,
                    &standing,
                    "keys.escrow",
                    actor.account().as_str(),
                ),
                company,
                Escrowed {
                    account: actor.account(),
                    recovery_key_id: recovery_key_id.as_str(),
                    wrap,
                },
                self.vaults.key(company)?,
            )
            .await
    }

    /// The recovery key the caller's own escrow is wrapped to, so their browser escrows again only when it moved.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] per the decision; [`Error::NotFound`] when the caller keeps no escrow here.
    pub async fn escrowed_to(&self, actor: &Actor, company: &CompanyId) -> Result<KeyId> {
        access::authorize(
            &self.database,
            &self.place,
            actor,
            company,
            Action::KeepKeys,
        )
        .await?;
        let row = self
            .repository
            .escrow_row(&self.place, company, actor.account())
            .await?
            .ok_or(Error::NotFound)?;
        KeyId::parse(&row.recovery_key_id).map_err(|_| Error::Unreadable {
            record: "escrow",
            reason: "the recovery key id did not read back as written".to_owned(),
        })
    }

    /// `account`'s escrow and the recovery key it is wrapped to, for an owner; `account` is told.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] per the decision (owners only); [`Error::NotFound`] when `account` keeps no escrow here.
    pub async fn escrow_of(
        &self,
        actor: &Actor,
        company: &CompanyId,
        account: &AccountId,
    ) -> Result<(KeyId, Vec<u8>)> {
        let standing = access::authorize(
            &self.database,
            &self.place,
            actor,
            company,
            Action::KeyRecovery,
        )
        .await?;
        let (key_id, wrap) = self
            .repository
            .read_escrow(
                &self.place,
                recorded(
                    actor,
                    company,
                    &standing,
                    "member.escrow.read",
                    account.as_str(),
                ),
                company,
                account,
                self.vaults.key(company)?,
            )
            .await?;
        let key_id = KeyId::parse(&key_id).map_err(|_| Error::Unreadable {
            record: "escrow",
            reason: "the recovery key id did not read back as written".to_owned(),
        })?;
        Ok((key_id, wrap))
    }

    /// The notices about the caller: every read of their escrow, oldest first.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] per the decision.
    pub async fn notices(&self, actor: &Actor, company: &CompanyId) -> Result<Vec<Notice>> {
        access::authorize(
            &self.database,
            &self.place,
            actor,
            company,
            Action::KeepKeys,
        )
        .await?;
        self.repository
            .notices(&self.place, company, actor.account())
            .await?
            .into_iter()
            .map(NoticeEntity::into_model)
            .collect()
    }
}
