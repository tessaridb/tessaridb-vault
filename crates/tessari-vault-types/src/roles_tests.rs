use super::Roles;
use crate::Role;

#[test]
fn a_set_holds_exactly_the_roles_named_and_writes_them_owner_first() {
    let set = Roles::parse(["member", "admin"]).expect("two roles");
    assert!(set.contains(Role::Admin) && set.contains(Role::Member) && !set.contains(Role::Owner));
    assert_eq!(set.words(), ["admin", "member"]);
    assert_eq!(Roles::parse(["admin", "admin"]), Roles::parse(["admin"]));
    assert_eq!(Roles::ALL.words(), ["owner", "admin", "member"]);
    assert_eq!(Roles::MANAGERS.words(), ["owner", "admin"]);
}

#[test]
fn an_empty_list_or_an_unknown_word_is_refused() {
    assert!(Roles::parse([]).is_err());
    assert!(Roles::parse(["admin", "auditor"]).is_err());
    assert!(Roles::parse(["Admin"]).is_err());
}
