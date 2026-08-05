#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "$0")/.." && pwd)
key_script="scripts/desktop-release-cache-key.py"
if command -v python3 >/dev/null 2>&1; then
  python_cmd=python3
else
  python_cmd=python
fi
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/repo"
(
  cd "$repo_root"
  git ls-files -z --cached --others --exclude-standard -- \
    '*Cargo.toml' \
    Cargo.lock \
    desktop/src-tauri/Cargo.lock \
    rust-toolchain.toml \
    .cargo/config.toml \
    scripts/desktop-release-cache-key.py |
    while IFS= read -r -d '' path; do
      [[ -e "$path" ]] && printf '%s\0' "$path"
    done |
    tar --null -T - -cf -
) | tar -C "$tmp/repo" -xf -
git -C "$tmp/repo" init -q
git -C "$tmp/repo" config core.autocrlf false
git -C "$tmp/repo" add .
cd "$tmp/repo"

args=(--platform Linux --target x86_64-unknown-linux-gnu --features mesh-llm --native-inputs ubuntu-24.04-mold)
original=$("$python_cmd" "$key_script" "${args[@]}")
"$python_cmd" - <<'PY'
from pathlib import Path
import re

manifest = Path("desktop/src-tauri/Cargo.toml")
manifest_text = manifest.read_text()
package = re.search(r'(?ms)^\[package\]\n.*?^version = "([^"]+)"', manifest_text)
if package is None:
    raise SystemExit("desktop package version not found")
current_version = package.group(1)
manifest.write_text(
    manifest_text[: package.start(1)] + "9.8.7" + manifest_text[package.end(1) :]
)

lock = Path("desktop/src-tauri/Cargo.lock")
text = lock.read_text()
start = text.index('name = "maju-desktop"')
version = text.index(f'version = "{current_version}"', start)
lock.write_text(
    text[:version]
    + 'version = "9.8.7"'
    + text[version + len(f'version = "{current_version}"') :]
)
PY
version_only=$("$python_cmd" "$key_script" "${args[@]}")
[[ "$original" == "$version_only" ]] || { echo "desktop version changed cache key" >&2; exit 1; }
printf '\n# dependency input\n' >> crates/maju-acp/Cargo.toml
dependency_changed=$("$python_cmd" "$key_script" "${args[@]}")
[[ "$original" != "$dependency_changed" ]] || { echo "dependency manifest did not change cache key" >&2; exit 1; }
[[ "$original" == desktop-rust-release-v1-Linux-x86_64-unknown-linux-gnu-* ]] || { echo "unexpected key: $original" >&2; exit 1; }
echo "desktop release cache key contract passed"
