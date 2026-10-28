//! Reading the store's answers into rows: the shape each statement answers in, mapped once for every domain.

use tessari_vault_infrastructure::tessaridb::{Answer, FromRecord};

use crate::{Error, Result};

fn unreadable(record: &'static str, reason: String) -> Error {
    Error::Unreadable { record, reason }
}

/// Every record the first answer holds, in the shape a `SELECT` answers in.
pub(crate) fn records<T: FromRecord>(answers: Vec<Answer>, record: &'static str) -> Result<Vec<T>> {
    let Some(answer) = answers.into_iter().next() else {
        return Ok(Vec::new());
    };
    answer
        .records_into::<T>()
        .map_err(|fault| unreadable(record, fault.to_string()))
}

/// The one record a statement addressed by id answered with, if any.
pub(crate) fn single<T: FromRecord>(
    answers: Vec<Answer>,
    record: &'static str,
) -> Result<Option<T>> {
    Ok(records(answers, record)?.pop())
}
