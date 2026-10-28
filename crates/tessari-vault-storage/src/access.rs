//! Who is calling, and their standing in a company, read in the same place every decision is taken.
//!
//! An [`Actor`] is made only by [`crate::sessions::SessionService::resolve`] from a session the store holds, so a
//! handler can never assert one. A company is reached through the caller's own membership row: a company id from a
//! request path is only a selection, and a caller with no active membership gets the answer a missing company
//! gets.

use std::collections::BTreeMap;

use tessari_vault_core::access::{Action, Decision, Standing, decide};
use tessari_vault_infrastructure::tessaridb::{Answer, Database, Place, Value};
use tessari_vault_types::{AccountId, CompanyId, CompanyState, MemberState, Role};

use crate::{Conflict, Error, Result};

/// A signed-in caller, as the session the store holds says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Actor {
    account: AccountId,
    platform_admin: bool,
    reason: String,
}

impl Actor {
    pub(crate) const fn new(account: AccountId, platform_admin: bool) -> Self {
        Self {
            account,
            platform_admin,
            reason: String::new(),
        }
    }

    /// The same caller, saying why they act: the audit trail records it with every change they make.
    #[must_use]
    pub fn with_reason(self, reason: String) -> Self {
        Self { reason, ..self }
    }

    /// Why the caller says they act; empty when they gave no reason.
    #[must_use]
    pub fn reason(&self) -> &str {
        &self.reason
    }

    /// The caller's account.
    #[must_use]
    pub const fn account(&self) -> &AccountId {
        &self.account
    }

    /// Whether the caller manages companies on this server.
    #[must_use]
    pub const fn is_platform_admin(&self) -> bool {
        self.platform_admin
    }

    /// The caller's platform authority, or the refusal a non-admin gets.
    pub(crate) fn platform(&self) -> Result<()> {
        if self.platform_admin {
            Ok(())
        } else {
            Err(Error::Refused(
                tessari_vault_core::access::Refusal::Forbidden,
            ))
        }
    }
}

/// The id of the membership row of `account` in `company`.
pub(crate) fn membership_id(company: &CompanyId, account: &AccountId) -> String {
    format!("{company}_{account}")
}

/// One bound parameter.
pub(crate) fn bind(name: &str, value: impl Into<Value>) -> (String, Value) {
    (name.to_owned(), value.into())
}

/// The standing `actor` holds in `company`, read now, and the decision on `action` taken from it.
///
/// # Errors
///
/// [`Error::Refused`] when the decision refuses; [`Error::Store`] when the standing could not be read, never a
/// refusal, because a store that did not answer decided nothing.
pub(crate) async fn authorize(
    database: &Database,
    place: &Place,
    actor: &Actor,
    company: &CompanyId,
    action: Action,
) -> Result<Standing> {
    let standing = read_standing(database, place, actor.account(), company).await?;
    require(standing.as_ref(), action)
}

/// The decision on `action` for a standing already read.
///
/// # Errors
///
/// [`Error::Refused`] when the decision refuses.
pub(crate) fn require(standing: Option<&Standing>, action: Action) -> Result<Standing> {
    match (decide(standing, action), standing) {
        (Decision::Permitted, Some(held)) => Ok(*held),
        (Decision::Refused(why), _) => Err(Error::Refused(why)),
        (Decision::Permitted, None) => Err(Error::Refused(
            tessari_vault_core::access::Refusal::NotVisible,
        )),
    }
}

/// The standing `actor` holds in `company`, read now.
///
/// # Errors
///
/// [`Error::Store`] when it could not be read.
pub(crate) async fn standing(
    database: &Database,
    place: &Place,
    actor: &Actor,
    company: &CompanyId,
) -> Result<Option<Standing>> {
    read_standing(database, place, actor.account(), company).await
}

/// The statement every write that rests on a decision starts with: it refuses the whole transaction when the
/// caller's membership or the company is no longer what the decision saw. Binds `$guard_m`, `$guard_c`,
/// `$guard_role`.
pub(crate) const GUARD: &str = "\
THROW 'standing changed' IF (GET memberships:$guard_m).member_role != $guard_role \
OR (GET memberships:$guard_m).state != 'active' OR (GET groups:$guard_c).state != 'active';";

/// The values [`GUARD`] binds and the audit event's: every guarded change is recorded on the company's trail in the
/// same transaction, as `action` done to `target`.
pub(crate) fn recorded(
    actor: &Actor,
    company: &CompanyId,
    standing: &Standing,
    action: &'static str,
    target: &str,
) -> Vec<(String, Value)> {
    let mut values = guard_values(actor, company, standing);
    values.extend(crate::audit::binds(
        actor,
        &crate::audit::Event {
            company: company.as_str(),
            action,
            target,
        },
    ));
    values
}

/// The values [`GUARD`] binds, for `standing` as `actor` holds it in `company`.
pub(crate) fn guard_values(
    actor: &Actor,
    company: &CompanyId,
    standing: &Standing,
) -> Vec<(String, Value)> {
    vec![
        bind("guard_m", membership_id(company, actor.account()).as_str()),
        bind("guard_c", company.as_str()),
        bind("guard_role", standing.role.word()),
    ]
}

async fn read_standing(
    database: &Database,
    place: &Place,
    account: &AccountId,
    company: &CompanyId,
) -> Result<Option<Standing>> {
    let answers = database
        .run_at(
            place,
            "RETURN { membership: (GET memberships:$m), company: (GET groups:$c) };",
            vec![
                bind("m", membership_id(company, account).as_str()),
                bind("c", company.as_str()),
            ],
        )
        .await?;
    let Some(Answer::Value {
        value: Value::Object(found),
        ..
    }) = answers.into_iter().next()
    else {
        return Err(unreadable("standing", "not one object"));
    };
    standing_from(&found)
}

fn standing_from(found: &BTreeMap<String, Value>) -> Result<Option<Standing>> {
    let (Some(Value::Object(membership)), Some(Value::Object(group))) =
        (found.get("membership"), found.get("company"))
    else {
        return Ok(None);
    };
    if word(group, "kind")? != "company" {
        return Ok(None);
    }
    Ok(Some(Standing {
        role: parsed::<Role>(membership, "member_role")?,
        member: parsed::<MemberState>(membership, "state")?,
        company: parsed::<CompanyState>(group, "state")?,
    }))
}

fn word<'a>(object: &'a BTreeMap<String, Value>, field: &'static str) -> Result<&'a str> {
    match object.get(field) {
        Some(Value::String(text)) => Ok(text),
        _ => Err(unreadable("standing", field)),
    }
}

fn parsed<T: std::str::FromStr>(
    object: &BTreeMap<String, Value>,
    field: &'static str,
) -> Result<T> {
    word(object, field)?
        .parse()
        .map_err(|_| unreadable("standing", field))
}

fn unreadable(record: &'static str, what: &str) -> Error {
    Error::Unreadable {
        record,
        reason: format!("{what} did not read back as written"),
    }
}

/// The conflict a `THROW` of ours names, or the error as it came.
///
/// The words are the ones the scripts in this crate throw; a refusal that starts with none of them is passed on
/// unchanged.
pub(crate) fn thrown(error: Error) -> Error {
    let Error::Store(tessari_vault_infrastructure::Error::Refused { message, .. }) = &error else {
        return error;
    };
    let named = [
        ("standing changed", Conflict::StandingChanged),
        ("last owner", Conflict::LastOwner),
        ("already a member", Conflict::AlreadyMember),
        ("group not empty", Conflict::GroupNotEmpty),
        ("wrong parent", Conflict::WrongParent),
        ("not a member", Conflict::NotAMember),
        ("already enrolled", Conflict::Enrolled),
        ("stale revision", Conflict::StaleRevision),
        ("out of order", Conflict::OutOfOrder),
        ("keys exist", Conflict::KeysExist),
        ("stale chain", Conflict::StaleChain),
        ("unknown key", Conflict::UnknownKey),
        ("no key", Conflict::NoKey),
        ("already shared", Conflict::AlreadyShared),
        ("stale shares", Conflict::StaleShares),
        ("rotation due", Conflict::RotationDue),
    ];
    if message.starts_with("not found") {
        return Error::NotFound;
    }
    named
        .into_iter()
        .find(|(words, _)| message.starts_with(words))
        .map_or(error, |(_, conflict)| Error::Conflict(conflict))
}
