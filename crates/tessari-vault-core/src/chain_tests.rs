//! The server's chain check against a chain the browser's own code wrote (`testdata/chain-vector.json`, made by
//! `appendStatement` and verified by the browser's `verifyChain`), and against every way of breaking it.

use serde_json::Value;

use super::{ChainRefused, Signed, verify};

struct Vector {
    company: String,
    owner: Vec<u8>,
    chain: Vec<(Vec<u8>, Vec<u8>)>,
    after_three: Value,
}

fn unhex(text: &str) -> Vec<u8> {
    text.as_bytes()
        .chunks(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).expect("ascii"), 16).expect("hex"))
        .collect()
}

fn pair(statement: &Value) -> (Vec<u8>, Vec<u8>) {
    let field = |name: &str| unhex(statement[name].as_str().expect("hex text"));
    (field("bytes"), field("signature"))
}

fn vector() -> Vector {
    let raw: Value =
        serde_json::from_str(include_str!("testdata/chain-vector.json")).expect("json");
    Vector {
        company: raw["company"].as_str().expect("company").to_owned(),
        owner: unhex(raw["owner_signing"].as_str().expect("owner")),
        chain: raw["chain"]
            .as_array()
            .expect("chain")
            .iter()
            .map(pair)
            .collect(),
        after_three: raw["after_three"].clone(),
    }
}

fn signed(chain: &[(Vec<u8>, Vec<u8>)]) -> Vec<Signed<'_>> {
    chain
        .iter()
        .map(|(bytes, signature)| Signed { bytes, signature })
        .collect()
}

#[test]
fn the_chain_the_browser_wrote_verifies_whole_and_at_every_length() {
    let vector = vector();
    assert_eq!(vector.chain.len(), 7);
    for length in 1..=vector.chain.len() {
        let chain = signed(&vector.chain[..length]);
        assert_eq!(
            verify(&vector.company, &vector.owner, &chain),
            Ok(()),
            "length {length}"
        );
    }
}

#[test]
fn a_signed_step_the_rules_forbid_is_refused_for_its_reason() {
    let vector = vector();
    for (name, reason) in [
        ("recovery_by_admin", ChainRefused::UnvouchedSigner),
        ("revoke_owner", ChainRefused::Broken),
        ("by_stranger", ChainRefused::UnvouchedSigner),
        ("rekey_then_old_admin", ChainRefused::UnvouchedSigner),
        ("revoked_admin", ChainRefused::UnvouchedSigner),
        ("fork_fourth", ChainRefused::Broken),
    ] {
        let mut chain = vector.chain[..3].to_vec();
        chain.extend(
            vector.after_three[name]
                .as_array()
                .expect("steps")
                .iter()
                .map(pair),
        );
        assert_eq!(
            verify(&vector.company, &vector.owner, &signed(&chain)),
            Err(reason),
            "{name}"
        );
    }
}

#[test]
fn a_changed_statement_or_signature_is_a_bad_signature() {
    let vector = vector();
    for (statement, in_signature, at) in
        [(0, false, 40), (4, false, 60), (2, true, 10), (6, true, 65)]
    {
        let mut chain = vector.chain.clone();
        let target = if in_signature {
            &mut chain[statement].1
        } else {
            &mut chain[statement].0
        };
        target[at] ^= 0x01;
        assert_eq!(
            verify(&vector.company, &vector.owner, &signed(&chain)),
            Err(ChainRefused::BadSignature),
            "statement {statement}, signature {in_signature}, byte {at}"
        );
    }
}

#[test]
fn a_chain_moved_reordered_shortened_or_pinned_to_another_key_is_broken() {
    let vector = vector();
    let whole = signed(&vector.chain);
    assert_eq!(
        verify("another-company", &vector.owner, &whole),
        Err(ChainRefused::Broken)
    );
    assert_eq!(
        verify(&vector.company, &[9; 32], &whole),
        Err(ChainRefused::Broken)
    );
    let mut swapped = vector.chain.clone();
    swapped.swap(1, 2);
    assert_eq!(
        verify(&vector.company, &vector.owner, &signed(&swapped)),
        Err(ChainRefused::Broken)
    );
    let mut gap = vector.chain.clone();
    gap.remove(1);
    assert_eq!(
        verify(&vector.company, &vector.owner, &signed(&gap)),
        Err(ChainRefused::Broken)
    );
    assert_eq!(
        verify(&vector.company, &vector.owner, &[]),
        Err(ChainRefused::Broken)
    );
    let raw: Value =
        serde_json::from_str(include_str!("testdata/chain-vector.json")).expect("json");
    let genesis = pair(&raw["genesis_naming_another_key"]);
    let genesis = [Signed {
        bytes: &genesis.0,
        signature: &genesis.1,
    }];
    assert_eq!(
        verify(&vector.company, &vector.owner, &genesis),
        Err(ChainRefused::Broken)
    );
}

#[test]
fn bytes_that_are_not_a_statement_are_malformed() {
    let vector = vector();
    let (bytes, signature) = &vector.chain[0];
    for cut in [0, 10, bytes.len() - 1] {
        let chain = [Signed {
            bytes: &bytes[..cut],
            signature,
        }];
        assert_eq!(
            verify(&vector.company, &vector.owner, &chain),
            Err(ChainRefused::Malformed),
            "cut {cut}"
        );
    }
}
