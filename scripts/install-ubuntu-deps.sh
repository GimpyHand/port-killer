#!/usr/bin/env bash
# Install port-killer system dependencies on Ubuntu 24.04 and 26.04.
set -euo pipefail

if [[ ! -f /etc/os-release ]]; then
  echo "error: /etc/os-release not found" >&2
  exit 1
fi

# shellcheck disable=SC1091
source /etc/os-release

if [[ "${ID:-}" != "ubuntu" && "${ID_LIKE:-}" != *debian* ]]; then
  echo "error: this script targets Ubuntu 24.04 / 26.04 (Debian-family)" >&2
  exit 1
fi

case "${VERSION_ID:-}" in
  24.04|26.04)
    echo "==> Ubuntu ${VERSION_ID} — installing dependencies"
    ;;
  *)
    echo "warning: tested on Ubuntu 24.04 and 26.04; continuing on ${PRETTY_NAME:-unknown}" >&2
    ;;
esac

PACKAGES=(
  build-essential
  iproute2
  pkg-config
  curl
  libgtk-4-dev
  libadwaita-1-dev
  waybar
)

sudo apt-get update
sudo DEBIAN_FRONTEND=noninteractive apt-get install -y "${PACKAGES[@]}"

echo
echo "System packages installed."
echo "Next: ./install --waybar"
