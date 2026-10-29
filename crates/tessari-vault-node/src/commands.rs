//! The operator commands' reports: what a rotation moved and what a deletion run deleted, printed for the operator
//! who ran them.

use tessari_vault_storage::Storage;
use tessari_vault_types::CompanyId;

use crate::StartError;

/// Run the rotation and print what it did; the report names counts and epochs, never a key.
pub(crate) async fn rotate(storage: &Storage) -> Result<&'static str, StartError> {
    let rotated = storage.rotation.rotate().await?;
    let remaining = storage.rotation.remaining().await?;
    println!(
        "rotated to epoch {}: {} company vaults moved, {} finished after a cut-short run, {} stranded; \
         {} TOTP seeds resealed, {} unopened",
        storage.rotation.current_epoch(),
        rotated.moved,
        rotated.repaired,
        rotated.stranded.len(),
        rotated.seeds_resealed,
        rotated.seeds_unopened,
    );
    println!(
        "still on each epoch: companies {}, verifiers {}, seeds {}",
        per_epoch(&remaining.companies),
        per_epoch(&remaining.verifiers),
        per_epoch(&remaining.seeds),
    );
    if rotated.stranded.is_empty() {
        Ok("rotate")
    } else {
        Err(StartError::Stranded(rotated.stranded.len()))
    }
}

/// `epoch:count` pairs, or `none`.
fn per_epoch(counts: &[(u32, usize)]) -> String {
    if counts.is_empty() {
        return "none".to_owned();
    }
    counts
        .iter()
        .map(|(epoch, count)| format!("{epoch}:{count}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// What `drop-due` deleted, and the rotation that bounds how long an older backup still opens the deleted vaults.
pub(crate) fn drop_report(dropped: &[CompanyId]) -> String {
    if dropped.is_empty() {
        return "no company deletion was due".to_owned();
    }
    let names = dropped
        .iter()
        .map(CompanyId::as_str)
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "deleted {} companies: {names}\nrotate the service key now (add an epoch, run `tessaridb-vault rotate`, then \
         remove the old epoch): until it leaves the key file a backup taken before the deletion still opens these vaults",
        dropped.len()
    )
}
