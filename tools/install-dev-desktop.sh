#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
binary="${project_root}/tools/launch-dev.sh"
target_dir="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
entry="io.github.balthier82.Wazzup.desktop"

mkdir -p "${target_dir}"
sed "s|^Exec=wazzup|Exec=${binary}|" "${project_root}/data/${entry}" > "${target_dir}/${entry}"
update-desktop-database "${target_dir}" 2>/dev/null || true
icons_dir="${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor"
cp -r "${project_root}/data/icons/hicolor/." "${icons_dir}/"
gtk-update-icon-cache --quiet --ignore-theme-index "${icons_dir}" 2>/dev/null || true
desktop-file-validate "${target_dir}/${entry}"
echo "Installed ${target_dir}/${entry}"
