//! The decision for one action in one company.

use tessari_vault_types::{CompanyState, MemberState, Role, Roles};

/// A caller's place in one company, as the store held it when the work began.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Standing {
    /// The role on the caller's membership of the company.
    pub role: Role,
    /// Whether that membership counts.
    pub member: MemberState,
    /// Whether the company is open.
    pub company: CompanyState,
    /// The service-key epoch the company's records vault is keyed under, read with the rest, so the key a request
    /// presents is the one the vault holds when the work began.
    pub key_epoch: u32,
}

/// What a caller asks to do inside a company. The roles an action touches are part of it, because making or
/// unmaking an owner is a different power from managing members.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// List the company's members and their roles.
    ReadMembers,
    /// List the company's departments and teams and who is in them.
    ReadGroups,
    /// Add an account to the company with `role`.
    AddMember {
        /// The role it is given.
        role: Role,
    },
    /// Change a member's role from `from` to `to`, or their state (then `from == to`).
    ChangeMember {
        /// The role the member holds now.
        from: Role,
        /// The role the member will hold.
        to: Role,
    },
    /// Remove a member who holds `role` from the company.
    RemoveMember {
        /// The role the member holds.
        role: Role,
    },
    /// Create a department or a team.
    CreateGroup,
    /// Rename a department or a team.
    RenameGroup,
    /// Delete a department or a team.
    DeleteGroup,
    /// Put a member into a department or a team.
    AddToGroup,
    /// Take a member out of a department or a team.
    RemoveFromGroup,
    /// Read the company's audit trail.
    ReadAudit,
    /// Keep a new record of a type whose records `allowed` may create.
    CreateRecord {
        /// The roles the record's type admits; a role reaching one of them is admitted too.
        allowed: Roles,
    },
    /// Define a record type of the company's own.
    DefineRecordType,
    /// Read a record; which records is decided on the record.
    ReadRecord,
    /// Write a new revision of a record; which records is decided on the record.
    ReviseRecord,
    /// Share a record with a group or a member; which records is decided on the record.
    ShareRecord,
    /// Delete a record with every revision, share and file: its author may, and an admin may delete anyone's.
    DeleteRecord {
        /// Whether the caller wrote the record.
        authored: bool,
    },
    /// Keep one's own key set in the company, and read it back.
    KeepKeys,
    /// Read the company's key directory and vouching chain.
    ReadKeys,
    /// Key a group, or wrap its key to members of its reach.
    KeyGroup,
    /// Keep the company's recovery key wrapped to its owners, and read one's own wrap of it.
    KeyRecovery,
    /// Extend the company's vouching chain; the `genesis` statement begins it.
    Vouch {
        /// Whether this statement begins the chain.
        genesis: bool,
    },
}

/// Why an action was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// The caller is not an active member of an open company. The answer is the one a company that does not exist
    /// gets, so it says nothing about whether this one does.
    NotVisible,
    /// The caller sees the company and their role does not include the action.
    Forbidden,
}

/// The answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// The action may go ahead.
    Permitted,
    /// It may not, and why.
    Refused(Refusal),
}

/// Whether a caller standing as `standing` (or not at all) in a company may take `action` there.
#[must_use]
pub fn decide(standing: Option<&Standing>, action: Action) -> Decision {
    let Some(held) = standing
        .filter(|held| held.member == MemberState::Active && held.company == CompanyState::Active)
    else {
        return Decision::Refused(Refusal::NotVisible);
    };
    let needed = match action {
        Action::CreateRecord { allowed } => {
            let admitted = [Role::Owner, Role::Admin, Role::Member]
                .into_iter()
                .any(|role| allowed.contains(role) && reaches(held.role, role));
            return if admitted {
                Decision::Permitted
            } else {
                Decision::Refused(Refusal::Forbidden)
            };
        }
        Action::ReadMembers
        | Action::ReadGroups
        | Action::ReadRecord
        | Action::ReviseRecord
        | Action::ShareRecord
        | Action::DeleteRecord { authored: true }
        | Action::KeepKeys
        | Action::ReadKeys => Role::Member,
        Action::AddMember { role: Role::Owner }
        | Action::RemoveMember { role: Role::Owner }
        | Action::Vouch { genesis: true }
        | Action::KeyRecovery
        | Action::ChangeMember {
            from: Role::Owner, ..
        }
        | Action::ChangeMember {
            to: Role::Owner, ..
        } => Role::Owner,
        Action::AddMember { .. }
        | Action::ChangeMember { .. }
        | Action::RemoveMember { .. }
        | Action::CreateGroup
        | Action::RenameGroup
        | Action::DeleteGroup
        | Action::AddToGroup
        | Action::RemoveFromGroup
        | Action::DeleteRecord { authored: false }
        | Action::DefineRecordType
        | Action::Vouch { genesis: false }
        | Action::KeyGroup
        | Action::ReadAudit => Role::Admin,
    };
    if reaches(held.role, needed) {
        Decision::Permitted
    } else {
        Decision::Refused(Refusal::Forbidden)
    }
}

/// Whether `held` includes everything `needed` may do: owner ⊇ admin ⊇ member.
const fn reaches(held: Role, needed: Role) -> bool {
    matches!(
        (held, needed),
        (Role::Owner, _) | (Role::Admin, Role::Admin | Role::Member) | (Role::Member, Role::Member)
    )
}

#[cfg(test)]
#[path = "decide_tests.rs"]
mod tests;
