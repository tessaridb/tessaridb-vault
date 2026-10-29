//! Group keys (KH-L1). Admins key a group and wrap its private key to the members inside its reach; every member
//! reads a group's public key and their own wrap of it. The reach is read from the store, never from the request.

use std::collections::HashSet;
use std::sync::Arc;

use tessari_vault_core::access::Action;
use tessari_vault_infrastructure::tessaridb::{Database, Place};
use tessari_vault_types::{AccountId, CompanyId, GroupId};

use super::entity::{GroupKey, MemberWrap, NewGroupKey, Revocation};
use super::repository::GroupKeyRepository;
use crate::access::{self, Actor, recorded};
use crate::keys::PublicKeys;
use crate::records::RecordVaults;
use crate::{Conflict, Error, Result};

/// Group keys.
pub struct GroupKeyService {
    repository: Arc<dyn GroupKeyRepository>,
    vaults: Arc<RecordVaults>,
    database: Database,
    place: Place,
}

/// Refuses unless every wrap names a distinct key id in `open`.
fn within(open: &HashSet<String>, wraps: &[MemberWrap]) -> Result<()> {
    let mut seen = HashSet::new();
    let fits = wraps.iter().all(|wrap| {
        let key = wrap.key_id().as_str();
        open.contains(key) && seen.insert(key)
    });
    if fits {
        Ok(())
    } else {
        Err(Error::Conflict(Conflict::OutsideReach))
    }
}

impl GroupKeyService {
    pub(crate) fn new(
        repository: Arc<dyn GroupKeyRepository>,
        vaults: Arc<RecordVaults>,
        database: Database,
        place: Place,
    ) -> Self {
        Self {
            repository,
            vaults,
            database,
            place,
        }
    }

    /// The key sets inside the group's reach that hold no wrap of its key yet; all of them for a key being replaced.
    async fn open(
        &self,
        company: &CompanyId,
        group: &GroupId,
        rotating: bool,
    ) -> Result<Vec<PublicKeys>> {
        let reach = self
            .repository
            .reach(&self.place, company, group)
            .await?
            .ok_or(Error::NotFound)?;
        let wrapped: HashSet<String> = match self.repository.get(&self.place, group).await? {
            Some(_) if !rotating => self
                .repository
                .wraps(&self.place, None, company, group, self.vaults.key(company)?)
                .await?
                .into_iter()
                .map(|(key, _)| key)
                .collect(),
            _ => HashSet::new(),
        };
        reach
            .into_iter()
            .filter(|key| !wrapped.contains(&key.key_id))
            .map(crate::keys::KeyEntity::into_public)
            .collect()
    }

    fn ids(keys: &[PublicKeys]) -> HashSet<String> {
        keys.iter()
            .map(|key| key.key_id.as_str().to_owned())
            .collect()
    }

    /// Key `group` with `key`, wrapped to members inside its reach; a key due for rotation is replaced, its old
    /// wraps dropped and the group's shares pointed at the new key.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] per the decision (admins key groups); [`Error::NotFound`] for a group that is not the
    /// company's; [`Conflict::KeysExist`] when the group is keyed and not due; [`Conflict::OutsideReach`] for a wrap
    /// to a key set outside the reach, or two to one.
    pub async fn keep(
        &self,
        actor: &Actor,
        company: &CompanyId,
        group: &GroupId,
        key: NewGroupKey,
    ) -> Result<()> {
        let standing = access::authorize(
            &self.database,
            &self.place,
            actor,
            company,
            Action::KeyGroup,
        )
        .await?;
        let existing = self.repository.get(&self.place, group).await?;
        let rotating = existing.as_ref().is_some_and(|entity| entity.rotate);
        let open = self.open(company, group, rotating).await?;
        // Keyed already is the answer before any wrap is judged; the script's own check holds under a race.
        if existing.is_some() && !rotating {
            return Err(Error::Conflict(Conflict::KeysExist));
        }
        within(&Self::ids(&open), key.wraps())?;
        let action = if rotating {
            "group_key.rotate"
        } else {
            "group_key.keep"
        };
        let guard = recorded(actor, company, &standing, action, group.as_str());
        let vault_key = self.vaults.key(company)?;
        if rotating {
            self.repository
                .rekey(&self.place, guard, company, group, &key, vault_key)
                .await
        } else {
            self.repository
                .keep(&self.place, guard, company, group, &key, vault_key)
                .await
        }
    }

    /// The wrap of `group`'s key to `account`'s key set, when there is one, ready to be taken off as they leave.
    pub(crate) async fn revocation(
        &self,
        company: &CompanyId,
        group: &GroupId,
        account: &AccountId,
    ) -> Result<Option<Revocation>> {
        if self.repository.get(&self.place, group).await?.is_none() {
            return Ok(None);
        }
        let Some(own) = self
            .repository
            .own_key(&self.place, company, account)
            .await?
        else {
            return Ok(None);
        };
        let wrapped = self
            .repository
            .wraps(&self.place, None, company, group, self.vaults.key(company)?)
            .await?
            .into_iter()
            .any(|(key, _)| key == own);
        if !wrapped {
            return Ok(None);
        }
        Ok(Some(Revocation {
            recipient: format!("member:{own}"),
            vault_key: self.vaults.key(company)?,
        }))
    }

    /// Add `wraps` of the group's key, each to a key set inside the reach that holds none yet.
    ///
    /// # Errors
    ///
    /// As [`GroupKeyService::keep`], and [`Error::NotFound`] for a group with no key.
    pub async fn add_wraps(
        &self,
        actor: &Actor,
        company: &CompanyId,
        group: &GroupId,
        wraps: Vec<MemberWrap>,
    ) -> Result<()> {
        let standing = access::authorize(
            &self.database,
            &self.place,
            actor,
            company,
            Action::KeyGroup,
        )
        .await?;
        let open = self.open(company, group, false).await?;
        if self.repository.get(&self.place, group).await?.is_none() {
            return Err(Error::NotFound);
        }
        within(&Self::ids(&open), &wraps)?;
        self.repository
            .add_wraps(
                &self.place,
                recorded(actor, company, &standing, "group_key.wrap", group.as_str()),
                company,
                group,
                &wraps,
                self.vaults.key(company)?,
            )
            .await
    }

    /// The key sets inside the group's reach still without a wrap of its key.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] per the decision; [`Error::NotFound`] for a group that is not the company's.
    pub async fn pending(
        &self,
        actor: &Actor,
        company: &CompanyId,
        group: &GroupId,
    ) -> Result<Vec<PublicKeys>> {
        access::authorize(
            &self.database,
            &self.place,
            actor,
            company,
            Action::KeyGroup,
        )
        .await?;
        self.open(company, group, false).await
    }

    /// The group's key, with the caller's own wrap of it when they hold one; the read is on the trail first.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] unless the caller is an active member; [`Error::NotFound`] for a group that is not the
    /// company's or has no key.
    pub async fn read(
        &self,
        actor: &Actor,
        company: &CompanyId,
        group: &GroupId,
    ) -> Result<GroupKey> {
        let standing = access::authorize(
            &self.database,
            &self.place,
            actor,
            company,
            Action::ReadGroups,
        )
        .await?;
        let entity = self
            .repository
            .get(&self.place, group)
            .await?
            .filter(|entity| entity.company == company.as_str())
            .ok_or(Error::NotFound)?;
        let mine = self
            .repository
            .own_key(&self.place, company, actor.account())
            .await?;
        let wraps = self
            .repository
            .wraps(
                &self.place,
                Some(recorded(
                    actor,
                    company,
                    &standing,
                    "group_key.read",
                    group.as_str(),
                )),
                company,
                group,
                self.vaults.key(company)?,
            )
            .await?;
        let wrap = mine.and_then(|mine| {
            wraps
                .into_iter()
                .find_map(|(key, wrap)| (key == mine).then_some(wrap))
        });
        Ok(GroupKey {
            group: group.clone(),
            key_id: entity.key_id()?,
            public: entity.public()?,
            wrap,
            rotation_due: entity.rotate,
        })
    }
}
