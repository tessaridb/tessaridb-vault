// The company's vouching chain as the page reads it, which owner key a new key set pins, and when an owner's browser
// still owes the chain its first statement.

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { decode, encode } from "../../src/base64url.ts";
import { genesisBody, needsGenesis, ownerPinFor, readChain } from "../../src/chain-model.ts";

const bytes = (length: number, fill: number) => new Uint8Array(length).fill(fill);
const item = (seq: number) => ({ seq, bytes: encode(bytes(40, seq)), signature: encode(bytes(64, seq)) });
const OWNER = bytes(32, 7);

describe("the chain the page reads", () => {
  it("reads an empty chain and a chain with its pinned owner key", () => {
    assert.deepEqual(readChain({ owner_signing: null, statements: [] }), { ownerSigning: null, statements: [] });
    const chain = readChain({ owner_signing: encode(OWNER), statements: [item(1), item(2)] });
    assert.deepEqual(chain?.ownerSigning, OWNER);
    assert.deepEqual(chain?.statements.map((statement) => statement.bytes), [bytes(40, 1), bytes(40, 2)]);
    assert.deepEqual(chain?.statements[1]?.signature, bytes(64, 2));
  });

  it("refuses an owner key without statements, statements without one, a gap, or a key of the wrong width", () => {
    assert.equal(readChain({ owner_signing: encode(OWNER), statements: [] }), null);
    assert.equal(readChain({ owner_signing: null, statements: [item(1)] }), null);
    assert.equal(readChain({ owner_signing: encode(OWNER), statements: [item(1), item(3)] }), null);
    assert.equal(readChain({ owner_signing: encode(OWNER), statements: [item(2)] }), null);
    assert.equal(readChain({ owner_signing: encode(bytes(31, 7)), statements: [item(1)] }), null);
    assert.equal(readChain({ owner_signing: encode(OWNER), statements: [{ ...item(1), signature: "" }] }), null);
    assert.equal(readChain({ statements: [] }), null);
  });
});

describe("the owner key a new key set pins", () => {
  const started = { ownerSigning: OWNER, statements: [] };
  const empty = { ownerSigning: null, statements: [] };

  it("is the key the chain began with, for every role", () => {
    assert.deepEqual(ownerPinFor(started, "owner"), OWNER);
    assert.deepEqual(ownerPinFor(started, "member"), OWNER);
  });

  it("is the person's own key when an owner finds no chain yet, and nothing for anyone else", () => {
    assert.equal(ownerPinFor(empty, "owner"), "self");
    assert.equal(ownerPinFor(empty, "admin"), "not-yet");
    assert.equal(ownerPinFor(empty, "member"), "not-yet");
  });
});

describe("the chain's first statement", () => {
  const keys = { signingPublic: bytes(32, 2), ownerSigningPublic: bytes(32, 2) };

  it("is owed by a key set that pinned itself while the chain is empty, and by no other", () => {
    assert.equal(needsGenesis({ ownerSigning: null, statements: [] }, keys), true);
    assert.equal(needsGenesis({ ownerSigning: bytes(32, 2), statements: [] }, keys), false);
    assert.equal(needsGenesis({ ownerSigning: null, statements: [] }, { ...keys, ownerSigningPublic: OWNER }), false);
  });

  it("is sent as statement 1 carrying the owner key it pins", () => {
    const body = genesisBody({ bytes: bytes(40, 1), signature: bytes(64, 1) }, OWNER);
    assert.deepEqual(Object.keys(body).sort(), ["bytes", "owner_signing", "seq", "signature"]);
    assert.equal(body["seq"], 1);
    assert.deepEqual(decode(String(body["bytes"])), bytes(40, 1));
    assert.deepEqual(decode(String(body["signature"])), bytes(64, 1));
    assert.deepEqual(decode(String(body["owner_signing"])), OWNER);
  });
});
