use super::super::KeyRing;
use super::super::service_key::tests::file;
use super::TotpSeal;
use crate::Error;

const SEED: &[u8] = b"12345678901234567890";

/// The seal under a ring holding one `(epoch, byte)` key per entry, the last current.
fn ring_seal(epochs: &[(u32, u8)]) -> TotpSeal {
    let text: String = epochs
        .iter()
        .map(|(epoch, byte)| format!("{epoch} {}\n", format!("{byte:02x}").repeat(32)))
        .collect();
    TotpSeal::new(&KeyRing::load(&file(&text, 0o600)).expect("a key file")).expect("a seal")
}

/// The seal under a service key whose every byte is `byte`, as epoch 1.
fn seal(byte: u8) -> TotpSeal {
    ring_seal(&[(1, byte)])
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
        sealed[..17].to_vec(),
        sealed[..sealed.len() - 1].to_vec(),
    ];
    for index in [0, 1, 4, 5, 17, sealed.len() - 1] {
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

#[test]
fn a_seed_names_the_epoch_it_was_sealed_under_and_opens_under_that_epoch_alone() {
    let sealed = seal(1).seal("acct-a", "totp-1", SEED).expect("sealed");
    assert_eq!(sealed.get(1..5), Some(&1_u32.to_be_bytes()[..]));
    let later = ring_seal(&[(1, 1), (2, 2)]);
    assert_eq!(
        later
            .open("acct-a", "totp-1", &sealed)
            .expect("an older epoch still opens")
            .as_slice(),
        SEED
    );
    let fresh = later.seal("acct-a", "totp-1", SEED).expect("sealed");
    assert_eq!(fresh.get(1..5), Some(&2_u32.to_be_bytes()[..]));
    assert_eq!(
        later
            .open("acct-a", "totp-1", &fresh)
            .expect("opened")
            .as_slice(),
        SEED
    );
    assert!(
        refused(ring_seal(&[(2, 2)]).open("acct-a", "totp-1", &sealed)),
        "an epoch the ring no longer holds does not open"
    );
    let mut renamed = fresh.clone();
    renamed.splice(1..5, 1_u32.to_be_bytes());
    assert!(
        refused(later.open("acct-a", "totp-1", &renamed)),
        "a seed relabelled to another epoch the ring holds does not open"
    );
    // Two epochs holding the same bytes: only the epoch in the associated data tells them apart.
    let twin = ring_seal(&[(1, 2), (2, 2)]);
    let mut relabelled = twin.seal("acct-a", "totp-1", SEED).expect("sealed");
    relabelled.splice(1..5, 1_u32.to_be_bytes());
    assert!(
        refused(twin.open("acct-a", "totp-1", &relabelled)),
        "the epoch is bound into the associated data, not only used to pick the key"
    );
}

#[test]
fn a_sealed_seed_says_which_epoch_sealed_it_and_the_seal_which_epoch_it_seals_under() {
    let later = ring_seal(&[(1, 1), (2, 2)]);
    assert_eq!(later.current_epoch(), 2);
    let older = seal(1).seal("acct-a", "totp-1", SEED).expect("sealed");
    let fresh = later.seal("acct-a", "totp-1", SEED).expect("sealed");
    assert_eq!(TotpSeal::epoch_of(&older), Some(1));
    assert_eq!(TotpSeal::epoch_of(&fresh), Some(2));
    for unreadable in [Vec::new(), vec![2, 0, 0], vec![1, 0, 0, 0, 2]] {
        assert_eq!(TotpSeal::epoch_of(&unreadable), None, "{unreadable:?}");
    }
}
