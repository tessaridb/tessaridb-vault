use super::{GroupKind, MemberState, Role, WordError};

#[test]
fn every_word_reads_back_as_itself() {
    for role in [Role::Owner, Role::Admin, Role::Member] {
        assert_eq!(role.word().parse::<Role>(), Ok(role));
    }
    for kind in [GroupKind::Company, GroupKind::Department, GroupKind::Team] {
        assert_eq!(kind.word().parse::<GroupKind>(), Ok(kind));
    }
}

#[test]
fn a_word_nobody_wrote_is_refused_rather_than_defaulted() {
    assert_eq!(
        "Owner".parse::<Role>(),
        Err(WordError {
            kind: "role",
            word: "Owner".to_owned()
        })
    );
    assert!("".parse::<MemberState>().is_err());
    assert!("superadmin".parse::<Role>().is_err());
}
