#!/usr/bin/env bash
# Repairs a PFX made by an older create-signing-cert.sh whose password.txt ended with a stray
# carriage return (\r). The PFX is re-encrypted with the same password minus the \r, keeping the
# same key and certificate, so the thumbprint does not change.
#
# Usage:  scripts/fix-pfx-password.sh [signing-dir]      (default: ~/pcscanner-signing)
# Then update both GitHub secrets again:
#   gh secret set SIGNING_CERT_PFX_BASE64 < ~/pcscanner-signing/pcscanner-codesign.pfx.base64
#   gh secret set SIGNING_CERT_PASSWORD   < ~/pcscanner-signing/password.txt

set -euo pipefail
export MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL="*"
umask 077

dir="${1:-$HOME/pcscanner-signing}"
NAME="pcscanner-codesign"
cd "$dir"

if ! grep -q $'\r' password.txt; then
  echo "password.txt has no carriage return; nothing to fix."
  exit 0
fi

cp "$NAME.pfx" "$NAME.pfx.bak"
cp password.txt password.txt.bak

NEWPASS="$(tr -d '\r\n' < password.txt)"
export NEWPASS

# Decrypt with the old password and re-encrypt with the clean one. The unencrypted key exists
# only briefly in this private folder and is removed on exit.
tmp="$NAME.tmp.pem"
trap 'rm -f "$tmp" "$NAME.new.pfx"' EXIT
openssl pkcs12 -in "$NAME.pfx" -passin file:password.txt -nodes -out "$tmp"
openssl pkcs12 -export -name "PC Scanner code signing" -in "$tmp" -out "$NAME.new.pfx" \
  -passout env:NEWPASS -keypbe AES-256-CBC -certpbe AES-256-CBC -macalg SHA256
rm -f "$tmp"

# Check the new file opens with the clean password before replacing anything.
openssl pkcs12 -in "$NAME.new.pfx" -passin env:NEWPASS -noout

mv "$NAME.new.pfx" "$NAME.pfx"
printf '%s' "$NEWPASS" > password.txt
openssl base64 -A -in "$NAME.pfx" > "$NAME.pfx.base64"
rm -f "$NAME.pfx.bak" password.txt.bak
unset NEWPASS

echo "Fixed. Same certificate:"
openssl pkcs12 -in "$NAME.pfx" -passin file:password.txt -nokeys 2>/dev/null \
  | openssl x509 -noout -fingerprint -sha1 | sed 's/.*=//; s/://g'
echo
echo "Now update both GitHub secrets (see the top of this script)."
