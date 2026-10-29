//! The recovery key and its owners: the key made with its owners' wraps, wraps for owners made after it, and the re-key once an owner stopped being one,
//! where the new key and its owners' wraps replace the old in one transaction with every escrow made to the old key.

use tessari_vault_constants::KEY_LIST_MAX;
use tessari_vault_infrastructure::tessaridb::{
    Database, FromRecord, MappingFault, Place, Row, Value, VaultKey,
};
use tessari_vault_types::{CompanyId, base64url};

use super::repository::Retiring;
use super::tessaridb::{OWNER, VID};
use crate::access::{GUARD, bind, thrown};
use crate::audit::RECORD;
use crate::group_keys::{MemberWrap, NewGroupKey, recipients};
use crate::records::RecordVaults;
use crate::{Result, answers};

/// One `escrows` row as the re-key reads it: whose escrow it is.
struct EscrowedAccount {
    account: String,
}

impl FromRecord for EscrowedAccount {
    fn from_row(mut row: Row) -> std::result::Result<Self, MappingFault> {
        Ok(Self {
            account: row.take("account")?,
        })
    }
}

pub(super) async fn rekey(
    database: &Database,
    place: &Place,
    mut guard: Vec<(String, Value)>,
    company: &CompanyId,
    retiring: Retiring<'_>,
    key: &NewGroupKey,
    vault_key: VaultKey,
) -> Result<()> {
    let vault = RecordVaults::name(company);
    let (added, wraps) = recipients(&vault, OWNER, key.wraps());
    guard.extend(wraps);
    guard.extend([
        bind("company", company.as_str()),
        bind("vid", VID),
        bind("retiring", retiring.key_id),
        bind("key_id", key.key_id().as_str()),
        bind("public", base64url::encode(key.public()).as_str()),
    ]);
    let mut dropped = String::new();
    for (index, account) in retiring.escrowed.iter().enumerate() {
        let name = format!("e{index}");
        dropped.push_str(&format!("DELETE {vault}:${name}; "));
        guard.push(bind(&name, format!("escrow.{account}").as_str()));
    }
    // The old wraps go with the record they were added to; the escrows made to the retiring key go with it.
    let script = format!(
        "BEGIN; {GUARD} THROW 'recovery keyed' IF (GET recovery_keys:$company).rotate != true \
         OR (GET recovery_keys:$company).key_id != $retiring; \
         UPDATE recovery_keys:$company SET key_id = $key_id, public = $public, rotate = false; \
         DELETE {vault}:$vid; CREATE {vault}:$vid = {{ body: $public }};{added} {dropped}\
         DELETE FROM escrows WHERE company = $company AND recovery_key_id = $retiring; {RECORD} COMMIT;"
    );
    database
        .transact_presenting(place, &script, "", guard, &vault, vault_key)
        .await
        .map_err(|error| thrown(error.into()))?;
    Ok(())
}

pub(super) async fn escrowed_to(
    database: &Database,
    place: &Place,
    company: &CompanyId,
    key_id: &str,
) -> Result<Vec<String>> {
    let answers = database
        .read_at(
            place,
            "SELECT account FROM escrows WHERE company = $company AND recovery_key_id = $key_id \
             LIMIT $limit USING INDEX by_recovery_key;",
            vec![
                bind("company", company.as_str()),
                bind("key_id", key_id),
                bind("limit", KEY_LIST_MAX),
            ],
        )
        .await?;
    let rows: Vec<EscrowedAccount> = answers::records(answers, "escrow")?;
    Ok(rows.into_iter().map(|row| row.account).collect())
}

pub(super) async fn add_wraps(
    database: &Database,
    place: &Place,
    mut guard: Vec<(String, Value)>,
    company: &CompanyId,
    wraps: &[MemberWrap],
    vault_key: VaultKey,
) -> Result<()> {
    let vault = RecordVaults::name(company);
    let (added, parameters) = recipients(&vault, OWNER, wraps);
    guard.extend(parameters);
    guard.push(bind("vid", VID));
    let script = format!("BEGIN; {GUARD}{added} {RECORD} COMMIT;");
    database
        .transact_presenting(place, &script, "", guard, &vault, vault_key)
        .await
        .map_err(|error| thrown(error.into()))?;
    Ok(())
}

pub(super) async fn keep(
    database: &Database,
    place: &Place,
    mut guard: Vec<(String, Value)>,
    company: &CompanyId,
    key: &NewGroupKey,
    vault_key: VaultKey,
) -> Result<()> {
    let vault = RecordVaults::name(company);
    let (added, wraps) = recipients(&vault, OWNER, key.wraps());
    guard.extend(wraps);
    guard.extend([
        bind("company", company.as_str()),
        bind("vid", VID),
        bind("key_id", key.key_id().as_str()),
        bind("public", base64url::encode(key.public()).as_str()),
    ]);
    let script = format!(
        "BEGIN; {GUARD} THROW 'recovery keyed' IF (GET recovery_keys:$company).key_id != NONE; \
         CREATE recovery_keys:$company = {{ company: $company, key_id: $key_id, public: $public, \
         rotate: false, created: time::now() }}; CREATE {vault}:$vid = {{ body: $public }};{added} {RECORD} COMMIT;"
    );
    database
        .transact_presenting(place, &script, "", guard, &vault, vault_key)
        .await
        .map_err(|error| thrown(error.into()))?;
    Ok(())
}
