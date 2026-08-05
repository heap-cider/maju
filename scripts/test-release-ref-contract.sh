#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
verify="${repo_root}/scripts/verify-release-ref.sh"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

git -C "$tmp" init -q
git -C "$tmp" config user.name test
git -C "$tmp" config user.email test@example.com
echo first >"$tmp/file"
git -C "$tmp" add file
git -C "$tmp" commit -qm first
git -C "$tmp" tag -m "desktop release" v1.2.3

(
  cd "$tmp"
  GITHUB_REF=refs/tags/v1.2.3 "$verify" v 1.2.3
)

if (
  cd "$tmp"
  GITHUB_REF=refs/heads/main "$verify" v 1.2.3
); then
  echo "branch-backed desktop release was accepted" >&2
  exit 1
fi

echo second >>"$tmp/file"
git -C "$tmp" commit -qam second
if (
  cd "$tmp"
  GITHUB_REF=refs/tags/v1.2.3 "$verify" v 1.2.3
); then
  echo "release accepted HEAD after the tag commit" >&2
  exit 1
fi

if grep -q 'inputs\.ref' \
  "$repo_root/.github/workflows/release.yml" \
  "$repo_root/.github/workflows/docker.yml"; then
  echo "publisher workflow still accepts a caller-selected source ref" >&2
  exit 1
fi

grep -q 'verify-release-ref\.sh' "$repo_root/.github/workflows/release.yml"
grep -q 'verify-release-ref\.sh' "$repo_root/.github/workflows/docker.yml"
grep -Fq 'tags: ["v[0-9]*"]' "$repo_root/.github/workflows/docker.yml"
grep -Fq 'scripts/verify-release-ref.sh v "$VERSION"' "$repo_root/.github/workflows/docker.yml"
if grep -q 'relay-v' \
  "$repo_root/.github/workflows/docker.yml" \
  "$repo_root/.github/workflows/auto-tag-on-release-pr-merge.yml" \
  "$repo_root/Justfile"; then
  echo "a separate relay-v release lane still exists" >&2
  exit 1
fi
"$repo_root/scripts/test-desktop-release-cache-key.sh"
"$repo_root/scripts/test-desktop-release-cache-workflow.sh"
grep -q 'test-release-ref-contract\.sh' "$repo_root/.github/workflows/ci.yml"
if grep -qE 'crates/maju-relay/Cargo\.toml|mobile/pubspec\.yaml' \
  "$repo_root/.github/workflows/release.yml"; then
  echo "desktop release still requires relay or mobile to share its version" >&2
  exit 1
fi
"$repo_root/scripts/test-signed-canary-contract.sh"
auto_tag="$repo_root/.github/workflows/auto-tag-on-release-pr-merge.yml"
if grep -q 'actions/create-github-app-token@\|MAJU_RELEASE_TAGGER_' "$auto_tag"; then
  echo "auto-tag still requires an unconfigured release GitHub App" >&2
  exit 1
fi
grep -q '^  actions: write' "$auto_tag"
grep -q '^  contents: write' "$auto_tag"
grep -q 'GH_TOKEN:.*secrets\.GITHUB_TOKEN' "$auto_tag"
grep -Fq 'git/refs' "$auto_tag"
grep -Fq 'if gh api "repos/$GITHUB_REPOSITORY/git/ref/tags/$TAG" --silent 2>/dev/null; then' "$auto_tag"
if grep -F 'git/ref/tags/$TAG' "$auto_tag" | grep -Fq '|| true'; then
  echo "auto-tag ignores a failed tag lookup, so a 404 body can look like an existing tag" >&2
  exit 1
fi
grep -Fq 'PUBLISHERS="release.yml docker.yml"' "$auto_tag"
grep -Fq 'PUBLISHERS="helm-chart.yml"' "$auto_tag"
grep -Fq 'PUBLISHERS="push-gateway-helm-chart.yml"' "$auto_tag"
grep -q 'gh workflow run' "$auto_tag"

echo "release ref contract passed"
