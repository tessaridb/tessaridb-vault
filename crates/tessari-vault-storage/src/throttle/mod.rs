//! Request counters per fixed window, in the `throttles` space.
//!
//! A window's counter is created with the window's expiry and `INCR` keeps it, so a counter disappears on its own
//! when its window ends and nothing has to sweep the space.

use tessari_vault_constants::RATE_WINDOW_SECONDS;
use tessari_vault_infrastructure::tessaridb::{Answer, Database, Number, Place, Value};

use crate::access::bind;
use crate::{Error, Result};

/// Request counters.
pub struct ThrottleService {
    database: Database,
    place: Place,
}

impl ThrottleService {
    pub(crate) const fn new(database: Database, place: Place) -> Self {
        Self { database, place }
    }

    /// Count one request against `key` and say whether it is within `limit` for the current window.
    ///
    /// `key` is built by the caller from verified values (an address, an account id) and is bound, never
    /// formatted into the statement.
    ///
    /// # Errors
    ///
    /// When the store fails; the caller refuses the request then rather than letting it through.
    pub async fn allow(&self, key: &str, limit: u32) -> Result<bool> {
        let window = Value::Duration {
            seconds: i64::try_from(RATE_WINDOW_SECONDS).unwrap_or(i64::MAX),
            nanos: 0,
        };
        // Requests from one address at once contend on this counter, and the store refuses the loser rather than
        // waiting: the pair is sent again, which counts once because `SET … IF ABSENT` changes nothing a second time.
        let answers = self
            .database
            .run_again_at(
                &self.place,
                "SET throttles:$key = 0 IF ABSENT EXPIRE $window; INCR throttles:$key;",
                vec![bind("key", key), ("window".to_owned(), window)],
            )
            .await?;
        let counted = answers.into_iter().rev().find_map(|answer| match answer {
            Answer::Value {
                value: Value::Number(Number::Integer(count)),
                ..
            } => Some(count),
            _ => None,
        });
        match counted {
            Some(count) => Ok(count <= i64::from(limit)),
            None => Err(Error::Unreadable {
                record: "throttle",
                reason: "a counter answered something other than a count".to_owned(),
            }),
        }
    }
}
