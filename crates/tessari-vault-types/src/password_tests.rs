use super::{Password, PasswordError};
use crate::Email;

fn email() -> Email {
    Email::parse("ada.lovelace@example.com").expect("an address")
}

fn check(text: &str) -> Result<(), PasswordError> {
    Password::parse(text, &email()).map(|_| ())
}

#[test]
fn eight_to_256_characters_counted_after_normalisation() {
    assert_eq!(check("seven77"), Err(PasswordError::Length));
    assert_eq!(check("eight888"), Ok(()));
    assert_eq!(check(&"x".repeat(256)), Ok(()));
    assert_eq!(check(&"x".repeat(257)), Err(PasswordError::Length));
    assert_eq!(
        check("cafe\u{301}cafe\u{301}"),
        Ok(()),
        "eight characters in NFC, ten code points typed"
    );
    assert_eq!(
        check("e\u{301}e\u{301}e\u{301}e\u{301}"),
        Err(PasswordError::Length),
        "four characters in NFC"
    );
}

#[test]
fn any_printable_character_and_space_is_allowed_and_no_composition_is_demanded() {
    for text in [
        "correct horse",
        "        a",
        "пароль-пароль",
        "🔑🔑🔑🔑🔑🔑🔑🔑",
        "aaaaaaaa",
    ] {
        assert_eq!(check(text), Ok(()), "{text:?}");
    }
    assert_eq!(check("tab\tinside"), Err(PasswordError::Control));
}

#[test]
fn the_accounts_address_or_its_local_part_is_refused_in_any_case() {
    for text in [
        "ada.lovelace@example.com",
        "my ADA.LOVELACE secret",
        "Ada.Lovelace2026",
    ] {
        assert_eq!(check(text), Err(PasswordError::Context), "{text:?}");
    }
    assert_eq!(
        check("ada and charles"),
        Ok(()),
        "a short part of the name is not the address"
    );
}

#[test]
fn a_password_never_prints() {
    let password = Password::parse("correct horse battery", &email()).expect("a password");
    assert_eq!(format!("{password:?}"), "Password(<redacted>)");
    assert_eq!(password.expose(), "correct horse battery");
}
