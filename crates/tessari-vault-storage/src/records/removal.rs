//! Deleting a record from TessariDB: every revision's sealed body and wraps leave the company's records vault, every
//! file's bytes leave the vault or the bucket, and the record's rows, revisions, shares and files go with them, all in
//! one transaction that refuses when the record moved after the caller's reach was decided.

use tessari_vault_infrastructure::tessaridb::{
    Database, FromRecord, MappingFault, Place, Row, Value,
};

use super::repository::Removal;
use super::tessaridb::{COMPANYS_RECORD, SHARES_UNCHANGED};
use super::vault::RecordVaults;
use crate::access::{GUARD, bind, thrown};
use crate::audit::RECORD;
use crate::files::{Kept, bucket_path, part_size, vault_entry};
use crate::{Error, Result, answers};

/// A file the record holds, read just before the delete.
struct Held {
    id: String,
    size: i64,
    kept: String,
}

impl FromRecord for Held {
    fn from_row(mut row: Row) -> std::result::Result<Self, MappingFault> {
        Ok(Self {
            id: row.take("file_id")?,
            size: row.take("size")?,
            kept: row.take("kept")?,
        })
    }
}

/// How many bucket parts a file of `size` bytes is written in.
pub(crate) fn parts(size: i64) -> Result<i64> {
    let part = part_size()?;
    part.checked_sub(1)
        .and_then(|spare| size.checked_add(spare))
        .and_then(|rounded| rounded.checked_div(part))
        .ok_or_else(|| Error::Unreadable {
            record: "file",
            reason: "a size past its range".to_owned(),
        })
}

/// The statements removing every file's bytes, with their bound values.
fn file_statements(
    vault: &str,
    company: &str,
    held: &[Held],
) -> Result<(String, Vec<(String, Value)>)> {
    let mut script = String::new();
    let mut parameters = Vec::new();
    for (index, file) in held.iter().enumerate() {
        if file.kept == Kept::Vault.word() {
            script.push_str(&format!(" DELETE {vault}:$file{index};"));
            parameters.push(bind(
                &format!("file{index}"),
                vault_entry(&file.id).as_str(),
            ));
            continue;
        }
        for part in 0..parts(file.size)? {
            script.push_str(&format!(" DELETE record_files:$file{index}_{part};"));
            parameters.push(bind(
                &format!("file{index}_{part}"),
                bucket_path(company, &file.id, part).as_str(),
            ));
        }
    }
    Ok((script, parameters))
}

pub(super) async fn delete(
    database: &Database,
    place: &Place,
    mut guard: Vec<(String, Value)>,
    removal: Removal<'_>,
) -> Result<()> {
    let (company, record) = (removal.company.as_str(), removal.record.as_str());
    let held: Vec<Held> = answers::records(
        database
            .read_at(
                place,
                "SELECT file_id, size, kept FROM files WHERE record = $record AND company = $company \
                 USING INDEX by_record;",
                vec![bind("record", record), bind("company", company)],
            )
            .await?,
        "file",
    )?;
    let vault = RecordVaults::name(removal.company);
    let mut revisions = String::new();
    for revision in 1..=removal.revision {
        revisions.push_str(&format!(" DELETE {vault}:$revision{revision};"));
        guard.push(bind(
            &format!("revision{revision}"),
            format!("{record}.{revision}").as_str(),
        ));
    }
    let (files, file_values) = file_statements(&vault, company, &held)?;
    guard.extend(file_values);
    let count = i64::try_from(held.len()).map_err(|_| Error::Unreadable {
        record: "file",
        reason: "more files than a count holds".to_owned(),
    })?;
    guard.extend([
        bind("company", company),
        bind("record", record),
        bind("expected", removal.revision),
        bind("share_version", removal.share_version),
        bind("files", count),
    ]);
    // A file started after the list was read would be left behind, so a changed count refuses as a stale record does.
    let script = format!(
        "BEGIN; {GUARD} {COMPANYS_RECORD} \
         THROW 'stale revision' IF (GET records:$record).revision != $expected; {SHARES_UNCHANGED} \
         THROW 'stale revision' IF array::len((SELECT file_id FROM files WHERE record = $record)) != $files; \
         {revisions}{files} \
         DELETE FROM revisions WHERE record = $record; DELETE FROM shares WHERE record = $record; \
         DELETE FROM files WHERE record = $record; DELETE records:$record; {RECORD} COMMIT;"
    );
    database
        .transact_presenting(place, &script, "", guard, &vault, removal.vault_key)
        .await
        .map_err(|error| thrown(error.into()))?;
    Ok(())
}
