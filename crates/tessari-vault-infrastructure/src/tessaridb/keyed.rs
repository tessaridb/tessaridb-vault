//! The vault frame's acts on a vault in key custody: declaring it under a key, and re-wrapping its key from one key to
//! another. Neither is a statement, so neither joins a transaction; the keys travel as frame fields and are never
//! stored.

use tessaridb_client::{Request, VaultKey};

use super::connection::KeyAct;
use super::{Database, Place};
use crate::{Error, Result};

impl Database {
    /// Declare the vault `vault` at `place` in key custody, its key wrapped under `key`, which is never stored.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidName`] when `vault` is not a plain name; [`Error::Refused`] when the node refuses, a name
    /// already taken included; [`Error::Unavailable`] when the store cannot be reached.
    pub async fn declare_keyed_vault(
        &self,
        place: &Place,
        vault: &str,
        key: &VaultKey,
    ) -> Result<()> {
        self.keyed(place, vault, KeyAct::Declare(key)).await
    }

    /// Re-wrap the key of the vault `vault` at `place` from `current` to `new`; no record is re-encrypted.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidName`] when `vault` is not a plain name; [`Error::Refused`] when the node refuses, a wrong
    /// `current` included (the node throttles those like a wrong passphrase); [`Error::Unavailable`] when the store
    /// cannot be reached.
    pub async fn change_keyed_vault(
        &self,
        place: &Place,
        vault: &str,
        current: &VaultKey,
        new: &VaultKey,
    ) -> Result<()> {
        self.keyed(place, vault, KeyAct::Change { current, new })
            .await
    }

    async fn keyed(&self, place: &Place, vault: &str, act: KeyAct<'_>) -> Result<()> {
        let (_slot, mut client, _) = self.converse(&Request::new(place.preamble())).await?;
        match client
            .keyed((place.namespace(), place.database()), vault, act)
            .await
        {
            Ok(()) => {
                self.release(client);
                Ok(())
            }
            Err(tessaridb_client::Error::Build(_)) => {
                self.release(client);
                Err(Error::InvalidName {
                    name: vault.to_owned(),
                })
            }
            Err(failure) => Err(self.failed(client, false, failure)),
        }
    }
}
