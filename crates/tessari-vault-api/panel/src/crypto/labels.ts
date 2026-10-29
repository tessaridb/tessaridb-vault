// Every domain-separation string the vault's format uses. Adding one is a format change, recorded in the key-hierarchy
// worksheet's label registry before it ships.

const PREFIX = "tessaridb-vault/v1/";

export const LABEL = {
  /** HKDF info for the unlock wrap key derived from the master key. */
  unlockWrap: `${PREFIX}unlock-wrap`,
  /** HKDF info for a revision's content key derived from its record key. */
  recordContent: `${PREFIX}record-content`,
  /** Associated-data prefix of a record body. */
  record: `${PREFIX}record`,
  /** HKDF info for a file's key derived from its record key. */
  file: `${PREFIX}file`,
  /** Associated-data prefix of every chunk of a file. */
  fileChunk: `${PREFIX}file-chunk`,
  /** Associated-data prefix of a membership private key wrapped under the unlock key. */
  memberKey: `${PREFIX}member-key`,
  /** Associated-data prefix of the company owner's public key, pinned under the unlock key in a member's key set. */
  ownerPin: `${PREFIX}owner-pin`,
  /** HPKE `info` prefix of a key wrap. */
  wrap: `${PREFIX}wrap`,
  /** Ed25519 domain prefix of a vouching statement. */
  statement: `${PREFIX}statement`,
  /** Ed25519 domain prefix of a revision signature. */
  revision: `${PREFIX}revision`,
  /** SHA-256 prefix of the key fingerprint two people compare before one vouches for the other's key. */
  fingerprint: `${PREFIX}fingerprint`,
  /** SHA-256 prefix of the code a member and an owner compare before a recovery hand-over. */
  handoffCode: `${PREFIX}handoff-code`,
} as const;

/** The format version every sealed value starts with. */
export const FORMAT_VERSION = 1;

/** The algorithm allow-list (key-hierarchy worksheet §3a). */
export const ALGORITHM = {
  recordBody: 0x01,
  keyWrap: 0x02,
  memberKey: 0x03,
  signature: 0x04,
  file: 0x05,
} as const;
