#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export CEF_PATH="${CEF_PATH:-$HOME/.local/share/cef}"

cargo build --manifest-path "${project_root}/Cargo.toml"
exec "${project_root}/tools/launch-dev.sh" "$@"
