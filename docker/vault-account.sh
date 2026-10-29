#!/bin/bash
# Declares TessariDB Vault's own database account, once, as the store's owner.
#
# The server signs in as an editor of its ONE namespace and can reach nothing else;
# it never signs in as the owner. The owner declares the namespace, the database and
# the account; every statement carries `IF NOT EXISTS`, so running this again changes
# nothing (a new password is set with `ALTER USER`, not by re-running it).
#
# The statement is written to a private file and run with `-f`, so the account's
# password never appears in a process's arguments. The owner's password reaches the
# client through `TESSARIDB_PASSWORD`, as the client requires.
set -euo pipefail

: "${TESSARIDB_AT:?}" "${TESSARIDB_OWNER:?}" "${TESSARIDB_PASSWORD:?}"
: "${VAULT_NAMESPACE:?}" "${VAULT_DATABASE:?}" "${VAULT_STORE_USER:?}" "${VAULT_STORE_PASSWORD:?}"

for name in "$VAULT_NAMESPACE" "$VAULT_DATABASE" "$VAULT_STORE_USER"; do
  [[ "$name" =~ ^[a-z][a-z0-9_]{0,62}$ ]] || { echo "not a name: $name" >&2; exit 1; }
done
# The password goes into the statement as text (DEFINE USER takes no parameter there),
# so a quote or a backslash would end it early: refuse rather than escape.
[[ "$VAULT_STORE_PASSWORD" != *"'"* && "$VAULT_STORE_PASSWORD" != *'\'* ]] \
  || { echo "VAULT_STORE_PASSWORD may not contain a quote or a backslash" >&2; exit 1; }
(( ${#VAULT_STORE_PASSWORD} >= 16 )) || { echo "VAULT_STORE_PASSWORD needs at least 16 characters" >&2; exit 1; }

script="$(mktemp)"
trap 'rm -f "$script"' EXIT
chmod 600 "$script"
cat > "$script" <<STATEMENTS
DEFINE NAMESPACE IF NOT EXISTS ${VAULT_NAMESPACE};
USE NAMESPACE ${VAULT_NAMESPACE};
DEFINE DATABASE IF NOT EXISTS ${VAULT_DATABASE};
USE NAMESPACE ${VAULT_NAMESPACE} DATABASE ${VAULT_DATABASE};
DEFINE USER IF NOT EXISTS ${VAULT_STORE_USER} ON NAMESPACE ${VAULT_NAMESPACE} ROLE editor PASSWORD '${VAULT_STORE_PASSWORD}';
STATEMENTS
exec tessaridb --at "$TESSARIDB_AT" --user "$TESSARIDB_OWNER" -f "$script"
