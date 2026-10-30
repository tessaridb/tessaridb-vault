# Changelog

All notable changes to TessariDB Vault are documented here. The format follows Keep a Changelog, and versions use the project's third-digit release scheme.

## [0.0.2] - 2026-10-31

### Added

- Optional TessariDB S3 storage for encrypted attachments through the official `tessaridb-s3-client` 0.2.1. Configure it with `TESSARIDB_VAULT_S3_ENDPOINT`, `_BUCKET`, `_ACCESS_KEY`, `_SECRET_KEY`, and optional `_REGION`; the existing vault and database-bucket backends remain the defaults.
- Unique object keys per upload attempt, metadata finalization after object upload, checked reads, and S3 object deletion for file and company removal.
- Contact built-in record type and Identity job, organization, website, and social-profile fields.
- Payment card type and billing-address fields.
- Public and private SSH key reveal and download for generated and imported keys.
- Operator configuration examples and migration guidance for optional S3 attachment storage.

### Fixed

- Explicitly empty `TESSARIDB_VAULT_S3_REGION` is rejected during configuration validation instead of failing later during request signing.

### Security

- S3 receives ciphertext only; plaintext attachment content and encryption keys never leave the browser for S3.
- Invalid S3 credentials, unavailable storage, incomplete uploads, and metadata finalization failures refuse to expose a completed file.

## [0.0.1] - 2026-10-30

### Added

- Initial TessariDB Vault release: corporate end-to-end encrypted records, sharing, embedded web panel, own sign-in with TOTP and passkeys, OIDC, organizational recovery, audit trail, and TessariDB-backed attachment storage.
- Multi-architecture container image and self-hosted Compose deployment.

[0.0.2]: https://github.com/tessaridb/tessaridb-vault/compare/v0.0.1...v0.0.2
[0.0.1]: https://github.com/tessaridb/tessaridb-vault/releases/tag/v0.0.1
