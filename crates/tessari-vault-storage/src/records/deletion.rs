//! Deleting a record: its author may, and a company admin may delete anyone's. A caller who may not and does not
//! reach the record is answered as for a record that does not exist.

use tessari_vault_core::access::Action;
use tessari_vault_types::{CompanyId, RecordId};

use super::RecordService;
use crate::access::{self, Actor, recorded};
use crate::records::repository::Removal;
use crate::{Error, Result};

impl RecordService {
    /// Remove the record with every revision, wrap, share and file, recorded on the company's trail with the
    /// caller's reason. Copies in backups taken earlier stay sealed until they expire.
    ///
    /// # Errors
    ///
    /// [`Error::NotFound`] for a record the caller does not reach and may not delete; [`Error::Refused`] for a
    /// caller who reaches it but neither wrote it nor administers the company; [`crate::Conflict::StaleRevision`]
    /// when it moved after it was read.
    pub async fn delete(
        &self,
        actor: &Actor,
        company: &CompanyId,
        record: &RecordId,
    ) -> Result<()> {
        let standing = access::authorize(
            &self.database,
            &self.place,
            actor,
            company,
            Action::ReadRecord,
        )
        .await?;
        let entity = self
            .repository
            .get(&self.place, company, record)
            .await?
            .ok_or(Error::NotFound)?;
        let authored = entity.author == actor.account().as_str();
        if let Err(refused) = access::require(Some(&standing), Action::DeleteRecord { authored }) {
            self.reach
                .of(company, actor.account(), &entity.id, &entity.author)
                .await?
                .ok_or(Error::NotFound)?;
            return Err(refused);
        }
        self.repository
            .delete(
                &self.place,
                recorded(actor, company, &standing, "record.delete", record.as_str()),
                Removal {
                    company,
                    record,
                    revision: entity.revision,
                    share_version: entity.share_version,
                    vault_key: self.vaults.key(company)?,
                },
            )
            .await
    }
}
