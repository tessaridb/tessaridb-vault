use tessari_vault_types::{CompanyState, MemberState, Role, Roles};

use super::{Action, Decision, Refusal, Standing, decide};

fn standing(role: Role) -> Standing {
    Standing {
        role,
        member: MemberState::Active,
        company: CompanyState::Active,
    }
}

const READS: [Action; 2] = [Action::ReadMembers, Action::ReadGroups];

/// What every active member may do with key sets: keep their own, read the company's directory and chain.
const KEYS: [Action; 2] = [Action::KeepKeys, Action::ReadKeys];

/// What every active member may do with records; which records is decided on the record itself.
const RECORDS: [Action; 3] = [
    Action::CreateRecord {
        allowed: Roles::ALL,
    },
    Action::ReadRecord,
    Action::ReviseRecord,
];

const MANAGING: [Action; 14] = [
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

const OWNER_ONLY: [Action; 6] = [
    Action::Vouch { genesis: true },
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
    if held.member != MemberState::Active || held.company != CompanyState::Active {
        return Decision::Refused(Refusal::NotVisible);
    }
    let touches_owner = match action {
        Action::AddMember { role } | Action::RemoveMember { role } => role == Role::Owner,
        Action::ChangeMember { from, to } => from == Role::Owner || to == Role::Owner,
        Action::Vouch { genesis } => genesis,
        _ => false,
    };
    let reads = matches!(
        action,
        Action::ReadMembers | Action::ReadGroups | Action::KeepKeys | Action::ReadKeys
    );
    let allowed = reads || held.role == Role::Owner || (held.role == Role::Admin && !touches_owner);
    if allowed {
        Decision::Permitted
    } else {
        Decision::Refused(Refusal::Forbidden)
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
                for company in [CompanyState::Active, CompanyState::Suspended] {
                    let held = Standing {
                        role,
                        member,
                        company,
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
    // 28 actions (13 without a role, 5 per role) under 12 standings.
    assert_eq!(compared, 28 * 12);
}
