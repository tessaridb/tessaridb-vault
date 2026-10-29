//! Vouching statements laid out and signed as the browser lays them out and signs them, so a test can extend a
//! company's chain the server verifies.

use ring::digest::{SHA256, digest};
use ring::rand::SystemRandom;
use ring::signature::{Ed25519KeyPair, KeyPair as _};
use serde_json::{Value, json};

use crate::records::encoded;

/// A signing key a test signs statements with.
pub(crate) struct Signer(Ed25519KeyPair);

impl Signer {
    pub(crate) fn new() -> Self {
        let pkcs8 = Ed25519KeyPair::generate_pkcs8(&SystemRandom::new()).expect("a key");
        Self(Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).expect("readable"))
    }

    pub(crate) fn public(&self) -> Vec<u8> {
        self.0.public_key().as_ref().to_vec()
    }
}

/// What a statement says: its kind's position (member 1, admin 2, group 3, revoke 4, recovery 5) and fields.
pub(crate) struct Says<'a> {
    pub(crate) kind: u8,
    pub(crate) subject: &'a str,
    pub(crate) agreement: &'a [u8],
    pub(crate) signing: &'a [u8],
}

/// A company's chain as a test grows it: each statement placed after the last and signed.
pub(crate) struct Growing {
    company: String,
    seq: u64,
    prev: [u8; 32],
}

fn field(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend(u32::try_from(bytes.len()).expect("short").to_be_bytes());
    out.extend(bytes);
}

impl Growing {
    pub(crate) fn new(company: &str) -> Self {
        Self {
            company: company.to_owned(),
            seq: 0,
            prev: [0; 32],
        }
    }

    /// The next statement as the request body `POST /api/companies/{company}/statements` takes, without advancing the chain.
    pub(crate) fn next(&self, says: &Says<'_>, by: &Signer) -> (Value, [u8; 32]) {
        let seq = self.seq.checked_add(1).expect("short chain");
        let mut bytes = b"tessaridb-vault/v1/statement".to_vec();
        bytes.extend([1, says.kind]);
        field(&mut bytes, self.company.as_bytes());
        bytes.extend(seq.to_be_bytes());
        bytes.extend(self.prev);
        field(&mut bytes, says.subject.as_bytes());
        field(&mut bytes, says.agreement);
        field(&mut bytes, says.signing);
        bytes.extend(by.public());
        let mut signature = vec![1, 0x04];
        signature.extend(by.0.sign(&bytes).as_ref());
        let mut hashed = bytes.clone();
        hashed.extend(&signature);
        let mut next_prev = [0; 32];
        next_prev.copy_from_slice(digest(&SHA256, &hashed).as_ref());
        let body =
            json!({ "seq": seq, "bytes": encoded(&bytes), "signature": encoded(&signature) });
        (body, next_prev)
    }

    /// Record that the statement `next` answered was accepted.
    pub(crate) fn accepted(&mut self, next_prev: [u8; 32]) {
        self.seq = self.seq.checked_add(1).expect("short chain");
        self.prev = next_prev;
    }
}
