//! Reading a stored record back: its string fields, the hex a sealed seed is kept in, and the clock.

use std::time::{SystemTime, UNIX_EPOCH};

use tessari_vault_infrastructure::tessaridb::{Answer, Value};

/// Seconds since the Unix epoch, for the TOTP step; a clock before 1970 reads as 0 and no code matches.
pub(crate) fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

pub(crate) fn unhex(text: &str) -> Option<Vec<u8>> {
    let digit = |byte: u8| {
        char::from(byte)
            .to_digit(16)
            .and_then(|d| u8::try_from(d).ok())
    };
    let (pairs, rest) = text.as_bytes().as_chunks::<2>();
    if !rest.is_empty() {
        return None;
    }
    pairs
        .iter()
        .map(|[high, low]| Some(digit(*high)? << 4 | digit(*low)?))
        .collect()
}

/// The string field `name` of an object answer.
pub(super) fn field(answer: Option<Answer>, name: &str) -> Option<String> {
    match answer {
        Some(Answer::Value {
            value: Value::Object(object),
            ..
        }) => match object.get(name) {
            Some(Value::String(text)) => Some(text.clone()),
            _ => None,
        },
        _ => None,
    }
}
