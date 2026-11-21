#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
engine_dir="${CEF_PATH:-$HOME/.local/share/cef}"

if [[ ! -f "${engine_dir}/libcef.so" ]]; then
    echo "Chromium engine not found in ${engine_dir}" >&2
    exit 1
fi

export CEF_PATH="${engine_dir}"
export WAZZUP_ENGINE_DIR="${engine_dir}"
export LD_LIBRARY_PATH="${engine_dir}${LD_LIBRARY_PATH:+:${LD_LIBRARY_PATH}}"

exec "${project_root}/target/debug/wazzup" "$@"
