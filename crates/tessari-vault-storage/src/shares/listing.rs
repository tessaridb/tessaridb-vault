//! Listing a record's shares to whoever may revise it: a revision must wrap its new record key to the author's key and
//! to every share's, so the browser needs exactly this set. It serves no decryption material, so like the members list
//! it is not on the trail.

use tessari_vault_core::access::Action;
use tessari_vault_types::{AccountId, CompanyId, GroupId, KeyId, RecordId};

use super::super::entity::{Access, ListedShare, ListedShares, ShareEntity, ShareTarget};
use super::ShareService;
use crate::access::{self, Actor};
use crate::{Error, Result};

fn unreadable(what: &str) -> Error {
    Error::Unreadable {
        record: "share",
        reason: format!("{what} did not read back as written"),
    }
}

fn listed(share: &ShareEntity) -> Result<ListedShare> {
    let (kind, key) = share
        .recipient
        .split_once(':')
        .ok_or_else(|| unreadable("its recipient"))?;
    let target = match kind {
        "group" => {
            ShareTarget::Group(GroupId::parse(&share.target).map_err(|_| unreadable("its group"))?)
        }
        "member" => ShareTarget::Member(
            AccountId::parse(&share.target).map_err(|_| unreadable("its member"))?,
        ),
        _ => return Err(unreadable("its recipient")),
    };
    Ok(ListedShare {
        target,
        access: Access::parse(&share.access).ok_or_else(|| unreadable("its access"))?,
        key_id: KeyId::parse(key).map_err(|_| unreadable("its key"))?,
    })
}

impl ShareService {
    /// The shares of `record` and its author's key, for a caller who may revise it.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] per the decision and for a caller who only reads the record; [`Error::NotFound`] for a
    /// record the caller does not reach.
    pub async fn list(
        &self,
        actor: &Actor,
        company: &CompanyId,
        record: &RecordId,
    ) -> Result<ListedShares> {
        access::authorize(
            &self.database,
            &self.place,
            actor,
            company,
            Action::ReviseRecord,
        )
        .await?;
        let (entity, reached) = self.records.reached(actor, company, record).await?;
        reached.editing()?;
        let author =
            AccountId::parse(&entity.author).map_err(|_| unreadable("the record's author"))?;
        let author_key = self
            .repository
            .member_key(&self.place, company, &author)
            .await?
            .map(|key| KeyId::parse(&key).map_err(|_| unreadable("the author's key")))
            .transpose()?;
        let shares = reached
            .shares
            .iter()
            .map(listed)
            .collect::<Result<Vec<_>>>()?;
        Ok(ListedShares {
            author,
            author_key,
            shares,
        })
    }
}
