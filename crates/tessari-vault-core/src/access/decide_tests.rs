use tessari_vault_types::{CompanyState, MemberState, Role, Roles};

use super::{Action, Decision, Refusal, Standing, decide};

fn standing(role: Role) -> Standing {
    Standing {
        role,
        member: MemberState::Active,
        company: CompanyState::Active,
        key_epoch: 1,
    }
}

const READS: [Action; 2] = [Action::ReadMembers, Action::ReadGroups];

/// What every active member may do with key sets: keep their own, read the company's directory and chain.
const KEYS: [Action; 2] = [Action::KeepKeys, Action::ReadKeys];

/// What every active member may do with records; which records is decided on the record itself.
const RECORDS: [Action; 5] = [
    Action::CreateRecord {
        allowed: Roles::ALL,
    },
    Action::ReadRecord,
    Action::ReviseRecord,
    Action::ShareRecord,
    Action::DeleteRecord { authored: true },
];

const MANAGING: [Action; 15] = [
    Action::DeleteRecord { authored: false },
    Action::KeyGroup,
    Action::Vouch { genesis: false },
    Action::DefineRecordType,
    Action::CreateRecord {
        allowed: Roles::MANAGERS,
    },
    Action::AddMember { role: Role::Member },
    Action::AddMember { role: Role::Admin },
    Action::ChangeMember {
        from: Role::Member,
        to: Role::Admin,
    },
    Action::RemoveMember { role: Role::Member },
    Action::CreateGroup,
    Action::RenameGroup,
    Action::DeleteGroup,
    Action::AddToGroup,
    Action::RemoveFromGroup,
    Action::ReadAudit,
];

const OWNER_ONLY: [Action; 7] = [
    Action::Vouch { genesis: true },
    Action::KeyRecovery,
    Action::AddMember { role: Role::Owner },
    Action::ChangeMember {
        from: Role::Owner,
        to: Role::Admin,
    },
    Action::ChangeMember {
        from: Role::Admin,
        to: Role::Owner,
    },
    Action::ChangeMember {
        from: Role::Member,
        to: Role::Owner,
    },
    Action::RemoveMember { role: Role::Owner },
];

fn every_action() -> Vec<Action> {
    READS
        .iter()
        .chain(&KEYS)
        .chain(&RECORDS)
        .chain(&MANAGING)
        .chain(&OWNER_ONLY)
        .copied()
        .collect()
}

#[test]
fn nobody_outside_the_company_is_shown_it_whatever_they_ask() {
    for action in every_action() {
        assert_eq!(
            decide(None, action),
            Decision::Refused(Refusal::NotVisible),
            "{action:?}"
        );
    }
}

#[test]
fn a_suspended_member_or_a_suspended_company_is_the_same_as_none() {
    for action in every_action() {
        for role in [Role::Owner, Role::Admin, Role::Member] {
            let mut suspended = standing(role);
            suspended.member = MemberState::Suspended;
            assert_eq!(
                decide(Some(&suspended), action),
                Decision::Refused(Refusal::NotVisible)
            );
            let mut closed = standing(role);
            closed.company = CompanyState::Suspended;
            assert_eq!(
                decide(Some(&closed), action),
                Decision::Refused(Refusal::NotVisible)
            );
        }
    }
}

#[test]
fn every_active_member_reads_the_structure() {
    for role in [Role::Owner, Role::Admin, Role::Member] {
        for action in READS {
            assert_eq!(decide(Some(&standing(role)), action), Decision::Permitted);
        }
    }
}

#[test]
fn every_active_member_keeps_their_keys_and_reads_the_companys() {
    for role in [Role::Owner, Role::Admin, Role::Member] {
        for action in KEYS {
            assert_eq!(decide(Some(&standing(role)), action), Decision::Permitted);
        }
    }
}

#[test]
fn every_active_member_keeps_records() {
    for role in [Role::Owner, Role::Admin, Role::Member] {
        for action in RECORDS {
            assert_eq!(decide(Some(&standing(role)), action), Decision::Permitted);
        }
    }
}

#[test]
fn a_record_of_a_restricted_type_is_created_only_by_the_roles_that_reach_it() {
    // A role reaches what every role below it may do, so a type allowed to admins is allowed to owners too.
    let admins = Roles::parse(["admin"]).expect("a set");
    let action = Action::CreateRecord { allowed: admins };
    for role in [Role::Owner, Role::Admin] {
        assert_eq!(
            decide(Some(&standing(role)), action),
            Decision::Permitted,
            "{role:?}"
        );
    }
    assert_eq!(
        decide(Some(&standing(Role::Member)), action),
        Decision::Refused(Refusal::Forbidden)
    );
}

#[test]
fn a_member_manages_nothing() {
    for action in MANAGING.iter().chain(&OWNER_ONLY) {
        assert_eq!(
            decide(Some(&standing(Role::Member)), *action),
            Decision::Refused(Refusal::Forbidden),
            "{action:?}"
        );
    }
}

#[test]
fn an_admin_manages_everything_but_owners() {
    for action in MANAGING {
        assert_eq!(
            decide(Some(&standing(Role::Admin)), action),
            Decision::Permitted,
            "{action:?}"
        );
    }
    for action in OWNER_ONLY {
        assert_eq!(
            decide(Some(&standing(Role::Admin)), action),
            Decision::Refused(Refusal::Forbidden),
            "{action:?}"
        );
    }
}

#[test]
fn an_owner_manages_everything() {
    for action in every_action() {
        assert_eq!(
            decide(Some(&standing(Role::Owner)), action),
            Decision::Permitted,
            "{action:?}"
        );
    }
}

/// The rules written a second time, as plainly as they can be said, and compared over every input.
fn reference(standing: Option<&Standing>, action: Action) -> Decision {
    let Some(held) = standing else {
        return Decision::Refused(Refusal::NotVisible);
    };
    if held.member != MemberState::Active
        || !matches!(held.company, CompanyState::Active | CompanyState::Dropping)
    {
        return Decision::Refused(Refusal::NotVisible);
    }
    let touches_owner = match action {
        Action::AddMember { role } | Action::RemoveMember { role } => role == Role::Owner,
        Action::ChangeMember { from, to } => from == Role::Owner || to == Role::Owner,
        Action::Vouch { genesis } => genesis,
        Action::DropCompany => true,
        _ => false,
    };
    let reads = matches!(
        action,
        Action::ReadMembers | Action::ReadGroups | Action::KeepKeys | Action::ReadKeys
    );
    let allowed = reads || held.role == Role::Owner || (held.role == Role::Admin && !touches_owner);
    // A company waiting to be deleted is read-only; its owners may still cancel.
    let readable = matches!(
        action,
        Action::ReadMembers
            | Action::ReadGroups
            | Action::ReadKeys
            | Action::ReadAudit
            | Action::ReadRecord
            | Action::DropCompany
    );
    if !allowed {
        Decision::Refused(Refusal::Forbidden)
    } else if held.company == CompanyState::Dropping && !readable {
        Decision::Refused(Refusal::ReadOnly)
    } else {
        Decision::Permitted
    }
}

#[test]
fn the_evaluator_agrees_with_the_plain_statement_of_the_rules_on_every_input() {
    let roles = [Role::Owner, Role::Admin, Role::Member];
    let mut actions = vec![
        Action::ReadMembers,
        Action::ReadGroups,
        Action::CreateGroup,
        Action::RenameGroup,
        Action::DeleteGroup,
        Action::AddToGroup,
        Action::RemoveFromGroup,
        Action::ReadAudit,
        Action::KeepKeys,
        Action::ReadKeys,
        Action::Vouch { genesis: false },
        Action::Vouch { genesis: true },
        Action::DropCompany,
        Action::KeyGroup,
    ];
    for role in roles {
        actions.push(Action::AddMember { role });
        actions.push(Action::RemoveMember { role });
        for to in roles {
            actions.push(Action::ChangeMember { from: role, to });
        }
    }
    let mut compared = 0_u32;
    for action in &actions {
        assert_eq!(decide(None, *action), reference(None, *action));
        for role in roles {
            for member in [MemberState::Active, MemberState::Suspended] {
                for company in [
                    CompanyState::Active,
                    CompanyState::Suspended,
                    CompanyState::Dropping,
                ] {
                    let held = Standing {
                        role,
                        member,
                        company,
                        key_epoch: 1,
                    };
                    assert_eq!(
                        decide(Some(&held), *action),
                        reference(Some(&held), *action),
                        "{held:?} {action:?}"
                    );
                    compared = compared.saturating_add(1);
                }
            }
        }
    }
    // 29 actions (14 without a role, 5 per role) under 18 standings.
    assert_eq!(compared, 29 * 18);
}

#[test]
fn a_company_waiting_to_be_deleted_is_read_only_and_its_owners_may_cancel() {
    let waiting = |role| Standing {
        company: CompanyState::Dropping,
        ..standing(role)
    };
    assert_eq!(
        decide(Some(&waiting(Role::Member)), Action::ReadRecord),
        Decision::Permitted
    );
    assert_eq!(
        decide(Some(&waiting(Role::Member)), Action::ReviseRecord),
        Decision::Refused(Refusal::ReadOnly)
    );
    assert_eq!(
        decide(Some(&waiting(Role::Admin)), Action::CreateGroup),
        Decision::Refused(Refusal::ReadOnly)
    );
    assert_eq!(
        decide(
            Some(&waiting(Role::Owner)),
            Action::CreateRecord {
                allowed: Roles::ALL
            }
        ),
        Decision::Refused(Refusal::ReadOnly),
        "no new record either"
    );
    assert_eq!(
        decide(Some(&waiting(Role::Owner)), Action::DropCompany),
        Decision::Permitted,
        "the owner cancels"
    );
    assert_eq!(
        decide(Some(&waiting(Role::Admin)), Action::DropCompany),
        Decision::Refused(Refusal::Forbidden)
    );
}
