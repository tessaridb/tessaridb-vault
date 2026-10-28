use super::super::ServiceKey;
use super::super::service_key::tests::file;
use super::TotpSeal;
use crate::Error;

const SEED: &[u8] = b"12345678901234567890";

/// The seal under a service key whose every byte is `byte`.
fn seal(byte: u8) -> TotpSeal {
    let key =
        ServiceKey::load(&file(&format!("{byte:02x}").repeat(32), 0o600)).expect("a key file");
    TotpSeal::new(&key).expect("a seal")
}

fn refused(result: crate::Result<zeroize::Zeroizing<Vec<u8>>>) -> bool {
    matches!(
        result,
        Err(Error::Custody {
            reason: "a sealed TOTP seed that does not open here"
        })
    )
}

#[test]
fn a_seed_opens_for_its_own_account_and_factor_only() {
    let seal = seal(1);
    let sealed = seal.seal("acct-a", "totp-1", SEED).expect("sealed");
    assert!(
        !sealed.windows(SEED.len()).any(|window| window == SEED),
        "no seed in the clear"
    );
    assert_ne!(
        sealed,
        seal.seal("acct-a", "totp-1", SEED).expect("sealed"),
        "a fresh nonce each time"
    );
    assert_eq!(
        seal.open("acct-a", "totp-1", &sealed)
            .expect("opened")
            .as_slice(),
        SEED
    );
    for (account, factor) in [
        ("acct-b", "totp-1"),
        ("acct-a", "totp-2"),
        ("acct-atotp-1", ""),
        ("", "acct-atotp-1"),
    ] {
        assert!(
            refused(seal.open(account, factor, &sealed)),
            "{account:?} {factor:?}"
        );
    }
}

#[test]
fn a_seed_sealed_under_another_service_key_does_not_open() {
    let sealed = seal(1).seal("acct-a", "totp-1", SEED).expect("sealed");
    assert!(refused(seal(2).open("acct-a", "totp-1", &sealed)));
}

#[test]
fn an_altered_truncated_or_unknown_format_seed_does_not_open() {
    let seal = seal(1);
    let sealed = seal.seal("acct-a", "totp-1", SEED).expect("sealed");
    let mut cases = vec![
        Vec::new(),
        sealed[..1].to_vec(),
        sealed[..13].to_vec(),
        sealed[..sealed.len() - 1].to_vec(),
    ];
    for index in [0, 1, 13, sealed.len() - 1] {
        let mut altered = sealed.clone();
        altered[index] ^= 0x01;
        cases.push(altered);
    }
    for case in cases {
        assert!(refused(seal.open("acct-a", "totp-1", &case)), "{case:?}");
    }
}

#[test]
fn the_seal_never_prints() {
    assert_eq!(format!("{:?}", seal(1)), "TotpSeal(<redacted>)");
}
