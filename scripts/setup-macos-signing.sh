#!/usr/bin/env bash
#
# Sets the macOS signing secrets the release pipeline needs.
#
# Run this yourself. It never echoes a secret, never takes one as a
# command-line argument (which would land in shell history and `ps` output),
# and never copies the certificate anywhere except into the GitHub secret.
#
#   ./scripts/setup-macos-signing.sh ~/Desktop/flume-signing.p12
#
# APPLE_SIGNING_IDENTITY and APPLE_TEAM_ID are already set. This covers the
# four that are genuinely sensitive.
#
# ---------------------------------------------------------------------------
# Why this does not export the certificate for you
#
# `security export` has no way to select a single identity -- it exports every
# identity of the requested type from the keychain. On a machine that also has
# an "Apple Development" certificate, that means shipping a second private key
# to CI that CI has no use for.
#
# Keychain Access can export exactly one certificate, so the export stays
# manual on purpose. Fewer keys leave the machine.
# ---------------------------------------------------------------------------

set -euo pipefail

REPO="adamgreenwell/flume"
EXPECTED_IDENTITY="Developer ID Application: Adam Greenwell (BJCR96U7RV)"

command -v gh >/dev/null || { echo "gh CLI not found."; exit 1; }

P12="${1:-}"
if [[ -z "$P12" ]]; then
  cat <<'USAGE'
Usage: ./scripts/setup-macos-signing.sh <path-to-p12>

First export the certificate, selecting only the Developer ID one:

  1. Open Keychain Access
  2. Category -> My Certificates
  3. Select "Developer ID Application: Adam Greenwell (BJCR96U7RV)"
     It must have a disclosure triangle: that is the private key, without
     which the certificate cannot sign.
  4. Right-click -> Export -> Personal Information Exchange (.p12)
  5. Choose a strong password; you will enter it again below

Then re-run this with the path to that file.
USAGE
  exit 1
fi

[[ -f "$P12" ]] || { echo "Not found: $P12"; exit 1; }

# --- Verify before uploading -------------------------------------------------
# A .p12 that turns out to be the wrong certificate is discovered otherwise
# only when notarization rejects the build, minutes into a release run.

echo "Checking $P12 ..."
if ! openssl pkcs12 -in "$P12" -nokeys -passin pass: -legacy 2>/dev/null | \
     openssl x509 -noout -subject 2>/dev/null | grep -q "Developer ID Application"; then
  echo
  echo "Could not confirm this contains a Developer ID Application certificate."
  echo "That is expected if the file is password-protected (it should be)."
  echo "Confirm in Keychain Access that you exported the right one."
  echo
  read -r -p "Continue? [y/N] " reply
  [[ "$reply" == "y" || "$reply" == "Y" ]] || exit 1
fi

echo "Expected identity: $EXPECTED_IDENTITY"
echo

# --- Upload ------------------------------------------------------------------

echo "Uploading the certificate..."
base64 -i "$P12" | gh secret set APPLE_CERTIFICATE -R "$REPO"

echo
echo "Enter the .p12 password you chose during export:"
gh secret set APPLE_CERTIFICATE_PASSWORD -R "$REPO"

echo
echo "Enter your Apple ID email:"
gh secret set APPLE_ID -R "$REPO"

echo
echo "Enter an APP-SPECIFIC password from appleid.apple.com."
echo "Not your account password. An app-specific password can be revoked on"
echo "its own and cannot be used to sign in to your account."
gh secret set APPLE_PASSWORD -R "$REPO"

# --- Confirm -----------------------------------------------------------------

echo
echo "Configured secrets:"
gh secret list -R "$REPO"

cat <<'DONE'

If all six are listed, the next tagged build signs and notarizes automatically.
Notarization adds a few minutes to each macOS leg.

Now delete the .p12, or move it somewhere encrypted. It contains a signing key.
DONE
