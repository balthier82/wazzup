#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
architecture="$(uname -m)"
engine_dir="${CEF_PATH:?set CEF_PATH to the exported CEF folder}"
version="${WAZZUP_VERSION:-$(sed -n 's/^version = "\(.*\)"/\1/p' "${project_root}/Cargo.toml" | head -n 1)}"
app_id="io.github.balthier82.Wazzup"
linuxdeploy_release="1-alpha-20251107-1"
gtk_plugin_commit="7a3fbc31a9e5075073ff8790f26effbac5f84453"
locales=(en-US it fr de es)

work="${project_root}/target/appimage"
app_dir="${work}/AppDir"
tools="${work}/tools"
output="${project_root}/dist"

fetch() {
    local destination="$1" address="$2"
    if [[ ! -s "${destination}" ]]; then
        curl --fail --location --silent --show-error --output "${destination}" "${address}"
        chmod +x "${destination}"
    fi
}

mkdir -p "${tools}" "${output}"
fetch "${tools}/linuxdeploy-${architecture}.AppImage" \
    "https://github.com/linuxdeploy/linuxdeploy/releases/download/${linuxdeploy_release}/linuxdeploy-${architecture}.AppImage"
fetch "${tools}/linuxdeploy-plugin-gtk.sh" \
    "https://raw.githubusercontent.com/linuxdeploy/linuxdeploy-plugin-gtk/${gtk_plugin_commit}/linuxdeploy-plugin-gtk.sh"

cargo build --release --manifest-path "${project_root}/Cargo.toml"

rm -rf "${app_dir}"
install -Dm755 "${project_root}/target/release/wazzup" "${app_dir}/usr/bin/wazzup"
install -d "${app_dir}/usr/lib/locales"
for resource in libcef.so libvk_swiftshader.so libvulkan.so.1 vk_swiftshader_icd.json \
    icudtl.dat v8_context_snapshot.bin resources.pak chrome_100_percent.pak chrome_200_percent.pak; do
    install -Dm644 "${engine_dir}/${resource}" "${app_dir}/usr/lib/${resource}"
done
for locale in "${locales[@]}"; do
    install -Dm644 "${engine_dir}/locales/${locale}.pak" "${app_dir}/usr/lib/locales/${locale}.pak"
done
strip --strip-unneeded "${app_dir}/usr/lib/libcef.so" "${app_dir}/usr/lib/libvk_swiftshader.so"
chmod 755 "${app_dir}/usr/lib/"*.so*

install -Dm644 "${project_root}/data/${app_id}.metainfo.xml" "${app_dir}/usr/share/metainfo/${app_id}.metainfo.xml"
cp -r "${project_root}/data/icons" "${app_dir}/usr/share/"

export PATH="${tools}:${PATH}"
export APPIMAGE_EXTRACT_AND_RUN=1
export DEPLOY_GTK_VERSION=4
export NO_STRIP=1

linuxdeploy="${tools}/linuxdeploy-${architecture}.AppImage"
"${linuxdeploy}" --appdir "${app_dir}" \
    --executable "${app_dir}/usr/bin/wazzup" \
    --desktop-file "${project_root}/data/${app_id}.desktop" \
    --icon-file "${project_root}/data/icons/hicolor/scalable/apps/${app_id}.svg" \
    --plugin gtk

hook="${app_dir}/apprun-hooks/linuxdeploy-plugin-gtk.sh"
sed -i -e '/^export GDK_BACKEND=/d' -e '/^export GTK_THEME=/d' "${hook}"

export LDAI_OUTPUT="${output}/Wazzup-${version}-${architecture}.AppImage"
export LDAI_UPDATE_INFORMATION="gh-releases-zsync|balthier82|wazzup|latest|Wazzup-*-${architecture}.AppImage.zsync"
"${linuxdeploy}" --appdir "${app_dir}" --output appimage

echo "Built ${LDAI_OUTPUT}"
