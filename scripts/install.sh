#!/usr/bin/env bash
# Legacy entrypoint — use ./install from repo root instead.
exec "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/install" "$@"
