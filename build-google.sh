#!/usr/bin/env bash
# Build clean with a Google Desktop OAuth client baked in.
# Your values are saved locally in ~/.config/clean/google-oauth.env (mode 600)
# and never sent anywhere.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ENV_FILE="${CLEAN_GOOGLE_ENV_FILE:-$HOME/.config/clean/google-oauth.env}"

if [[ ! -f "$ENV_FILE" ]]; then
  echo "Google Desktop OAuth client setup"
  echo "Create one at https://console.cloud.google.com/apis/credentials"
  echo "  → Enable the Gmail API, then create an OAuth client of type 'Desktop app'."
  echo
  read -rp "Client ID: " CLIENT_ID
  read -rsp "Client secret: " CLIENT_SECRET
  echo
  if [[ -z "$CLIENT_ID" || -z "$CLIENT_SECRET" ]]; then
    echo "Both values are required." >&2
    exit 1
  fi
  mkdir -p "$(dirname "$ENV_FILE")"
  umask 077
  printf 'CLEAN_GOOGLE_CLIENT_ID=%q\nCLEAN_GOOGLE_CLIENT_SECRET=%q\n' \
    "$CLIENT_ID" "$CLIENT_SECRET" > "$ENV_FILE"
  echo "Saved to $ENV_FILE"
fi

# shellcheck disable=SC1090
set -a
source "$ENV_FILE"
set +a

echo "Building clean with the embedded Google client…"
cd "$ROOT"
CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-8}" cargo build --release --locked

echo
echo "Done. Run it with:  ./target/release/clean"
echo "Edit $ENV_FILE any time to change the client, then re-run this script."
