# Releasing Maju

Maju has one bundled public release lane. Mobile candidates outside the bundled
Android APK keep their immutable tags cut directly from remote `main`:

| Lane | Entry point | Artifact |
|------|-------------|----------|
| Maju bundle | `just release-desktop` | Windows installer, Android APK, Linux relay archive, and `ghcr.io/heap-cider/maju` image |
| Mobile | `scripts/mobile-release.sh candidate X.Y.Z` | Exact `mobile-vX.Y.Z-rc.N` source identity |

The ordinary `vX.Y.Z` tag identifies every artifact in the Maju bundle. Mobile
candidates derive both source and marketing version from their exact candidate
tag. The mobile handoff to the private `maju-releases` pipeline remains manual
because OSS CI cannot trigger private CI.

## Quick Start

```sh
# Maju release (next patch version)
just release-desktop

# Explicit version
just release-desktop 0.4.0

# Publish the next mobile candidate from the exact current remote main commit
scripts/mobile-release.sh candidate 0.5.0
```

Maju releases use metadata PRs. Mobile candidates do not. Each
`mobile-vX.Y.Z-rc.N` tag is an immutable candidate and the artifact of record.
There is no mobile release branch, stable mobile tag alias, finalization step,
or mobile GitHub Release.

---

## How It Works

### Maju bundle

1. **`just release-desktop`** runs locally on `main`, creates or updates a
   `version-bump/<version>` PR, bumps the desktop manifests, regenerates
   lockfiles, and updates `CHANGELOG.md`.
2. **Merge the PR.** `auto-tag-on-release-pr-merge` pushes `v<version>`.
3. **The tag triggers both publishers.** `release.yml` publishes the Windows
   installer, Android APK, and standalone Linux relay archive. `docker.yml`
   publishes the same relay source as an amd64/arm64 GHCR image under the full
   semver, major/minor, major, SHA, and stable `latest` aliases. Matching
   `debug-` tags retain symbols for profiling.
   Android's `versionCode` is derived monotonically from the release semver, so
   a signed GitHub APK installs over an older Maju APK without deleting app data.
4. **First publication only:** change the new `maju` package visibility to
   Public in GitHub Packages. Public GHCR images can then be pulled by a VPS
   without registry credentials.

### Mobile

1. **Publish a candidate.** From a clean checkout whose `origin` is the
   canonical `heap-cider/maju` repository, run
   `scripts/mobile-release.sh candidate X.Y.Z`. The script resolves and fetches
   the exact current `origin/main` commit, derives the next number from exact
   remote tags for that marketing version, and publishes an annotated
   `mobile-vX.Y.Z-rc.N` tag there through the dedicated `maju-release-bot`
   GitHub App. It never uses the operator's checked-out commit and never moves
   an existing candidate.
2. **Build the exact tag.** Enter the candidate tag as `mobile_ref` in the
   private Maju mobile Buildkite pipeline. OSS CI deliberately cannot trigger
   that private pipeline. The tag supplies both source commit and release
   version. Flutter receives clean marketing version `X.Y.Z`; Buildkite's
   monotonically increasing build number supplies the platform build number.
3. **Promote tested artifacts.** Promote the already-built signed artifact for
   each platform through its store workflow. Record the exact tag with the
   build or rollout record. No source ref is changed and no final build is cut.

The iOS and Android artifacts for one marketing version may come from different
RC tags. For example, iOS can ship `mobile-v0.5.0-rc.2` while Android ships
`mobile-v0.5.0-rc.3`. Each platform's exact candidate tag is its source record.
There is intentionally no single selected or final candidate for the marketing
version.

The simplification trades away a separate stabilization line. Unrelated commits
that reach `main` become part of every later candidate, and there is no retained
hotfix branch or branch-ancestry history. Add a dedicated hotfix flow later if a
release actually needs isolation from `main`.

`mobile/pubspec.yaml` keeps `0.0.0+1` only as a valid, visibly non-release
fallback for local development and validation builds. Release jobs always
inject both version fields. `mobile/CHANGELOG.md` is retained as historical
release data. It is not a release ledger for this flow.

---

## Version Sources

| Lane | Release version authority |
|------|---------------------------|
| Maju bundle | `desktop/package.json` and synchronized desktop manifests |
| Relay crate metadata | `crates/maju-relay/Cargo.toml` |
| Mobile | Exact `mobile-vX.Y.Z-rc.N` remote tag |

`just bump-desktop-version <version>` updates the desktop manifests and
regenerates their lockfiles. The relay crate version remains internal package
metadata; public relay archives and images take the immutable Maju tag version.
Mobile has no bump recipe or release-metadata PR.

---

## Signed macOS Canary

Use the manual **Signed macOS Canary** workflow when you need an Apple Silicon
build of current `main` for explicit testing without publishing a release:

```sh
gh workflow run signed-macos-canary.yml --repo heap-cider/maju --ref main
```

The workflow derives a `-test.<run-number>` version, signs and notarizes the
DMG, verifies it with Gatekeeper, and uploads it as a short-lived Actions
artifact with seven-day retention. Because this is a public repository, any
signed-in GitHub user can download that artifact while it exists; it is
unpublished, not private. The workflow has no release permissions, does not
create or move tags, and cannot update `maju-desktop-latest` or `latest.json`.

Download the artifact from the completed run:

```sh
gh run download <run-id> --repo heap-cider/maju --name <artifact-name>
```

The workflow intentionally accepts only `main`. Use the normal release process
for distributable builds or builds from an immutable release tag.

---

## Manual Release Retry

The **Release** workflow's manual dispatch is only a retry mechanism for an
existing immutable `v<version>` tag. Select that tag in the ref picker and
provide the matching semver version without the `v` prefix. It cannot build
from `main` or another caller-selected source ref.

Mobile intentionally has no branch or arbitrary-ref fallback. The private
Buildkite pipeline accepts only an exact candidate tag.

---

## Internal Releases

For mobile, trigger the private
[Release Mobile pipeline](https://buildkite.com/runway/maju-mobile-releases) with
an exact RC tag for the platform build being cut. For desktop, use
[Release Desktop](https://buildkite.com/runway/sprout-releases). See the
[maju-releases README](https://github.com/squareup/maju-releases#cutting-a-release)
for the private pipeline contract.

---

## What Gets Published

Maju publishes two GitHub releases plus a GHCR image:

1. **`v<version>`**: the user-facing release with Windows, Android, and Linux
   relay downloads; the same tag publishes `ghcr.io/heap-cider/maju:<version>`.
2. **`maju-desktop-latest`**: the rolling auto-updater release.

Mobile publishes only annotated `mobile-vX.Y.Z-rc.N` git tags. Store artifacts
and rollout records retain the exact tag they used. Mobile does not publish a
GitHub Release or a stable `mobile-vX.Y.Z` alias.

---

## Platform Support

The public release builds a Windows x86_64 installer, Android APK, standalone
Linux x86_64 relay archive, and an amd64/arm64 relay container manifest. macOS
and iOS are not public Maju release targets.

---

## Prerequisites

- **Write access** to the `heap-cider/maju` GitHub repository
- An `origin` remote whose configured URL is the canonical `heap-cider/maju`
  repository
- `gh` CLI version 2.87.0 or newer, authenticated with permission to dispatch
  the candidate workflow
- Release tag ruleset [`14378754`](https://github.com/heap-cider/maju/rules/14378754)
  active for `mobile-v*`, with creation, update, deletion, and non-fast-forward
  protections and `maju-release-bot` as its sole always-bypass actor
- The `maju-release-bot` App credentials configured for GitHub Actions
- The following **GitHub Actions secrets** must also be configured for the
  desktop release lane:

  | Secret | Purpose |
  |--------|---------|
  | `MAJU_UPDATER_PUBLIC_KEY` | Tauri updater public key (minisign) |
  | `TAURI_SIGNING_PRIVATE_KEY` | Tauri updater private key |
  | `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Password for the private key |

Mobile candidate publication requires workflow-dispatch access and the existing
release App because strict tag protection denies direct human creation. The App
must be installed on `heap-cider/maju`, have Contents write and Metadata read, and
retain an `always` bypass on the immutable `mobile-v*` tag rules. It does not
require GitHub Releases permissions, repository Administration permission, or a
mobile release-branch ruleset. The publisher validates the App token's effective
`current_user_can_bypass` value rather than reading the ruleset's hidden bypass
actor list.

---

## Troubleshooting

### `just release-desktop` fails with "must be on main branch"
Switch to `main` and pull latest before running the release recipe.

### `just release-desktop` fails with "working tree is dirty"
Commit or stash your changes before running the release recipe.

### New commits land after publishing a mobile candidate

Run `scripts/mobile-release.sh candidate <version>` again after the intended
fix reaches remote `main`. It publishes a new immutable RC tag at the new exact
remote commit. Continue referring to each tested or shipped platform artifact by
its own exact tag.

### `scripts/mobile-release.sh candidate` fails because `main` moved during publication

The App-backed workflow may already have published the requested immutable RC
at the prior `main` tip before the operator command detects the race. Do not
move or delete that tag, and do not treat it as the candidate for current
`main`. Inspect the run URL from the command output, then rerun
`scripts/mobile-release.sh candidate <version>` to publish the next RC from the
new current `main` tip.

### A mobile candidate command selects the wrong RC number

Do not retry by moving or deleting a tag. Inspect the exact remote `mobile-v*`
tags and resolve the unexpected state. Candidate numbers are monotonically
increasing remote identities.

### A mobile candidate publication is rejected by repository rules

Confirm `maju-release-bot` remains the sole always-bypass actor for the active
`mobile-v*` ruleset and that its Actions credentials are available. Do not grant
direct human creation or weaken update or deletion protection. Existing
candidate tags must remain immutable.

### Auto-updater reports "no update available"
Verify that the `maju-desktop-latest` release exists and contains a
valid `latest.json`. The manifest covers all four platform keys
(`darwin-aarch64`, `darwin-x86_64`, `linux-x86_64`,
`windows-x86_64`); a missing entry usually means that platform's
release job failed. Check the workflow run.
