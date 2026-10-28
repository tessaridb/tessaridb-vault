//! Which records an account reaches, and with what access: as their author, through a share to them, or through a
//! share to a group they are in: the company group, a group they are directly in, or the group that one sits in.
//! Read from the store on every call, never from the request.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use tessari_vault_infrastructure::tessaridb::Place;
use tessari_vault_types::{AccountId, CompanyId, KeyId};

use super::entity::{Access, ShareEntity};
use super::repository::ShareRepository;
use crate::{Conflict, Error, Result};

/// What an account holds on one record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Reached {
    /// The widest access any path gives.
    pub(crate) access: Access,
    /// Every share of the record.
    pub(crate) shares: Vec<ShareEntity>,
    /// The targets a share to which reaches the account.
    pub(crate) targets: HashSet<String>,
}

impl Reached {
    /// The recipient on the record's revision the account can open, and its wrap: the wrap to its own key `own_key`
    /// first, else the one to the key of a share that reaches it.
    pub(crate) fn wrap_for(
        &self,
        own_key: Option<&str>,
        recipients: &mut HashMap<String, Vec<u8>>,
    ) -> Option<(String, Vec<u8>)> {
        own_key
            .map(|key| format!("member:{key}"))
            .into_iter()
            .chain(
                self.shares
                    .iter()
                    .filter(|share| self.targets.contains(&share.target))
                    .map(|share| share.recipient.clone()),
            )
            .find_map(|name| recipients.remove(&name).map(|wrap| (name, wrap)))
    }

    /// Refuses unless the account may change the record: readers read.
    pub(crate) const fn editing(&self) -> Result<()> {
        match self.access {
            Access::Edit => Ok(()),
            Access::Read => Err(Error::Refused(
                tessari_vault_core::access::Refusal::Forbidden,
            )),
        }
    }
}

impl Reached {
    /// Whether `wraps` names exactly the recipients a revision by the key `reviser` must keep: the author's key and
    /// every share's, but the reviser's own, which the revision's body carries.
    pub(crate) fn keeps(
        &self,
        author_key: Option<&str>,
        reviser: &str,
        wraps: &[(String, Vec<u8>)],
    ) -> bool {
        let mut required: HashSet<String> = self
            .shares
            .iter()
            .map(|share| share.recipient.clone())
            .collect();
        required.extend(author_key.map(|key| format!("member:{key}")));
        required.remove(reviser);
        let named: HashSet<&str> = wraps.iter().map(|(name, _)| name.as_str()).collect();
        named.len() == wraps.len() && named == required.iter().map(String::as_str).collect()
    }
}

/// Reaching records through their authors and shares.
pub(crate) struct Reach {
    repository: Arc<dyn ShareRepository>,
    place: Place,
}

impl Reach {
    pub(crate) fn new(repository: Arc<dyn ShareRepository>, place: Place) -> Self {
        Self { repository, place }
    }

    /// The account, the company group, the company's groups the account is directly in, and the groups those sit
    /// in, which is a team's department or the company again.
    pub(crate) async fn targets(
        &self,
        company: &CompanyId,
        account: &AccountId,
    ) -> Result<HashSet<String>> {
        let mut targets = HashSet::from([account.as_str().to_owned(), company.as_str().to_owned()]);
        for group in self.repository.direct_groups(&self.place, account).await? {
            if group.company == company.as_str() {
                targets.insert(group.group);
                targets.insert(group.parent);
            }
        }
        Ok(targets)
    }

    /// What `account` holds on `record`, which `author` wrote; `None` when no path reaches it.
    pub(crate) async fn of(
        &self,
        company: &CompanyId,
        account: &AccountId,
        record: &str,
        author: &str,
    ) -> Result<Option<Reached>> {
        let shares = self.repository.of_record(&self.place, record).await?;
        let targets = self.targets(company, account).await?;
        let mut widest = (author == account.as_str()).then_some(Access::Edit);
        for share in shares
            .iter()
            .filter(|share| targets.contains(&share.target))
        {
            let access = Access::parse(&share.access).ok_or_else(|| Error::Unreadable {
                record: "share",
                reason: "its access did not read back as written".to_owned(),
            })?;
            widest = widest.max(Some(access));
        }
        Ok(widest.map(|access| Reached {
            access,
            shares,
            targets,
        }))
    }

    /// The ids of `company`'s records shared with any target reaching `account`, but those in `excluding`, each read
    /// through the target index.
    pub(crate) async fn shared(
        &self,
        company: &CompanyId,
        account: &AccountId,
        excluding: &HashSet<String>,
    ) -> Result<Vec<String>> {
        let mut records = HashSet::new();
        for target in &self.targets(company, account).await? {
            records.extend(
                self.repository
                    .shared_with(&self.place, company, target)
                    .await?,
            );
        }
        Ok(records
            .into_iter()
            .filter(|id| !excluding.contains(id))
            .collect())
    }

    /// Refuses a revision by `account` under `reviser_key` unless that key is theirs and `wraps` names exactly the
    /// recipients the record keeps but theirs: its author's key and every share's (see [`Reached::keeps`]).
    pub(crate) async fn revisable(
        &self,
        company: &CompanyId,
        account: &AccountId,
        author: &str,
        reached: &Reached,
        reviser_key: &KeyId,
        wraps: &[(String, Vec<u8>)],
    ) -> Result<()> {
        // The wraps are judged against the reviser's own key, so a key not theirs refuses first; the write checks it
        // again under a race.
        if self.member_key(company, account).await?.as_deref() != Some(reviser_key.as_str()) {
            return Err(Error::Conflict(Conflict::UnknownKey));
        }
        let author = AccountId::parse(author).map_err(|_| Error::Unreadable {
            record: "record",
            reason: "its author did not read back as written".to_owned(),
        })?;
        let author_key = self.member_key(company, &author).await?;
        let reviser = format!("member:{}", reviser_key.as_str());
        if reached.keeps(author_key.as_deref(), &reviser, wraps) {
            Ok(())
        } else {
            Err(Error::Conflict(Conflict::StaleShares))
        }
    }

    /// The id of the key set `account` keeps in `company`.
    pub(crate) async fn member_key(
        &self,
        company: &CompanyId,
        account: &AccountId,
    ) -> Result<Option<String>> {
        self.repository
            .member_key(&self.place, company, account)
            .await
    }
}
