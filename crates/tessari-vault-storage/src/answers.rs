//! Reading the store's answers into rows: the shape each statement answers in, mapped once for every domain.

use tessari_vault_infrastructure::tessaridb::{
    Answer, FromRecord, MappingFault, Row, StoreRecordId, Value,
};

use crate::{Error, Result};

/// Take a native record reference as its textual id for the domain's typed identifiers.
pub(crate) fn reference(row: &mut Row, field: &str) -> std::result::Result<String, MappingFault> {
    let value: Value = row.take(field)?;
    match value {
        Value::Record(reference) => match reference.id {
            StoreRecordId::Text(id) => Ok(id),
            _ => Err(MappingFault::WrongType {
                expected: "a textual record identity",
                found: "a non-textual record identity",
            }),
        },
        Value::String(id) => Ok(id),
        _ => Err(MappingFault::WrongType {
            expected: "a record reference",
            found: "another value",
        }),
    }
    .map_err(|cause| MappingFault::InField {
        name: field.to_owned(),
        cause: Box::new(cause),
    })
}

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
