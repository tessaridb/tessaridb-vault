//! A revision opened by the store: the revealed body, the recipient set and the revision row, read back into parts.

use std::collections::HashMap;

use tessari_vault_infrastructure::tessaridb::{Answer, Value};
use tessari_vault_types::{KeyId, base64url};

use super::entity::{OpenedRevision, Sealed};
use crate::shares::Reached;
use crate::{Error, Result};

impl OpenedRevision {
    /// The revision's sealed parts with the wrap the caller opens, and the recipient it is kept under: the wrap to
    /// their own key `own_key` first, else one to the key of a share that reaches them.
    pub(crate) fn for_caller(
        mut self,
        reached: &Reached,
        own_key: Option<&str>,
        rotate: bool,
    ) -> Result<(String, Sealed)> {
        let Some((recipient, wrap)) = reached.wrap_for(own_key, &mut self.recipients) else {
            // A group key replaced since the latest revision: the next revision wraps to the new one.
            return Err(if rotate {
                Error::Conflict(crate::Conflict::RotationDue)
            } else {
                unreadable("a wrap reaching the caller")
            });
        };
        let sealed = Sealed::new(self.body, self.signature, self.author_key, wrap)
            .map_err(|_| unreadable("the sealed parts"))?;
        Ok((recipient, sealed))
    }
}

fn unreadable(what: &str) -> Error {
    Error::Unreadable {
        record: "revision",
        reason: format!("{what} did not read back as written"),
    }
}

/// The object an answer holds.
fn object(answer: Option<Answer>, what: &str) -> Result<Value> {
    match answer {
        Some(Answer::Value {
            value: value @ Value::Object(_),
            ..
        }) => Ok(value),
        _ => Err(unreadable(what)),
    }
}

/// The text field `field` of an object.
fn text<'a>(object: &'a Value, field: &str) -> Result<&'a str> {
    match object {
        Value::Object(fields) => match fields.get(field) {
            Some(Value::String(text)) => Ok(text),
            _ => Err(unreadable(field)),
        },
        _ => Err(unreadable(field)),
    }
}

/// Every recipient of a set, which answers as `{ recipients: { <name>: <bytes> } }`.
fn recipients(answer: Option<Answer>) -> Result<HashMap<String, Vec<u8>>> {
    let Value::Object(mut answer) = object(answer, "recipients")? else {
        return Err(unreadable("recipients"));
    };
    let Some(Value::Object(named)) = answer.remove("recipients") else {
        return Err(unreadable("recipients"));
    };
    named
        .into_iter()
        .map(|(name, material)| match material {
            Value::Bytes(material) => Ok((name, material)),
            _ => Err(unreadable("a wrapped key")),
        })
        .collect()
}

/// The revision from the last three answers: the revealed body, the recipient set, and the revision row.
pub(crate) fn opened_from(mut answers: Vec<Answer>) -> Result<OpenedRevision> {
    let row = object(answers.pop(), "revision row")?;
    let recipients = recipients(answers.pop())?;
    let revealed = object(answers.pop(), "revealed body")?;
    Ok(OpenedRevision {
        body: base64url::decode(text(&revealed, "body")?).map_err(|_| unreadable("body"))?,
        signature: base64url::decode(text(&row, "signature")?)
            .map_err(|_| unreadable("signature"))?,
        author_key: KeyId::parse(text(&row, "author_key")?)
            .map_err(|_| unreadable("author key"))?,
        recipients,
    })
}
