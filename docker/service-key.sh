#!/bin/bash
# Writes the service key once, into the volume the server reads it from.
#
# The key is 32 random bytes as 64 hex characters. It derives every company's vault
# key, the password pepper and the TOTP seal, so it is made here, never passed in an
# environment variable, and kept only in this volume: back the volume up apart from the
# database, because a database restored without its service key cannot be opened.
# An existing key is left alone, so starting again never replaces it.
set -euo pipefail

key=/keys/service-key
if [[ -s "$key" ]]; then
  echo "service key present"
  exit 0
fi
umask 077
od -An -tx1 -N32 /dev/urandom | tr -d ' \n' > "$key.new"
echo >> "$key.new"
# 65532 is the user the server's image runs as; others may not read the key.
chown 65532:65532 "$key.new"
chmod 400 "$key.new"
mv "$key.new" "$key"
echo "service key written"
