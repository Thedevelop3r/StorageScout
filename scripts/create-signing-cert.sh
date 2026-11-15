#!/usr/bin/env bash
# Creates the self-signed code signing certificate used to sign StorageScout releases.
#
# Usage:   scripts/create-signing-cert.sh [output-dir]
# Needs:   OpenSSL 3 (included with Git for Windows; run it from Git Bash).
#
# Output (default: ~/storagescout-signing, OUTSIDE the repository):
#   storagescout-codesign.pfx         private key + certificate. SECRET: never commit or share it.
#   storagescout-codesign.pfx.base64  the same file as base64, for the GitHub secret.
#   storagescout-codesign.cer         public certificate. Safe to publish (copied to signing/).
#   password.txt                   the PFX password. SECRET.
#
# Then add two repository secrets (Settings > Secrets and variables > Actions):
#   SIGNING_CERT_PFX_BASE64   contents of storagescout-codesign.pfx.base64
#   SIGNING_CERT_PASSWORD     contents of password.txt
# For example with the GitHub CLI:
#   gh secret set SIGNING_CERT_PFX_BASE64 < ~/storagescout-signing/storagescout-codesign.pfx.base64
#   gh secret set SIGNING_CERT_PASSWORD   < ~/storagescout-signing/password.txt
# Keep a backup of the output folder somewhere safe and offline.

set -euo pipefail
# Stop Git Bash / MSYS from turning "/CN=..." into a Windows path.
export MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL="*"

SUBJECT="/CN=Bilal Amjad/O=Bilal Amjad (StorageScout)"
DAYS=1825   # 5 years
NAME="storagescout-codesign"

out="${1:-$HOME/storagescout-signing}"
repo="$(cd "$(dirname "$0")/.." && pwd)"

case "$(cd "$(dirname "$out")" 2>/dev/null && pwd)/" in
  "$repo"/*) echo "Refusing to write the private key inside the repository: $out" >&2; exit 1 ;;
esac
if [ -e "$out/$NAME.pfx" ]; then
  echo "$out/$NAME.pfx already exists; not overwriting it." >&2
  exit 1
fi

mkdir -p "$out"
chmod 700 "$out" 2>/dev/null || true
cd "$out"

# Random password for the PFX. Windows builds of OpenSSL end lines with \r\n, so strip both.
openssl rand -base64 33 | tr -d '\r\n' > password.txt
chmod 600 password.txt 2>/dev/null || true

# Key and self-signed certificate restricted to code signing.
openssl req -quiet -x509 -newkey rsa:4096 -sha256 -days "$DAYS" -nodes \
  -keyout "$NAME.key.pem" -out "$NAME.crt.pem" \
  -subj "$SUBJECT" \
  -addext "basicConstraints=critical,CA:FALSE" \
  -addext "keyUsage=critical,digitalSignature" \
  -addext "extendedKeyUsage=critical,codeSigning" \
  -addext "subjectKeyIdentifier=hash"

# PFX for signtool (AES-256, readable by Windows 10/11).
openssl pkcs12 -export -name "StorageScout code signing" \
  -inkey "$NAME.key.pem" -in "$NAME.crt.pem" -out "$NAME.pfx" \
  -passout file:password.txt \
  -keypbe AES-256-CBC -certpbe AES-256-CBC -macalg SHA256
rm -f "$NAME.key.pem"

openssl x509 -in "$NAME.crt.pem" -outform DER -out "$NAME.cer"
openssl base64 -A -in "$NAME.pfx" > "$NAME.pfx.base64"
rm -f "$NAME.crt.pem"

# Publish the public certificate in the repository.
mkdir -p "$repo/signing"
cp "$NAME.cer" "$repo/signing/$NAME.cer"

echo
echo "Created in $out:"
ls -l "$out"
echo
echo "SHA-1 thumbprint (shown by Windows):"
openssl x509 -inform DER -in "$NAME.cer" -noout -fingerprint -sha1 | sed 's/.*=//; s/://g'
echo "SHA-256 fingerprint:"
openssl x509 -inform DER -in "$NAME.cer" -noout -fingerprint -sha256 | sed 's/.*=//'
echo
echo "Public certificate copied to signing/$NAME.cer - commit it and put the thumbprint in SIGNING.md."
