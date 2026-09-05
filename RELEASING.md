# Releasing Maju

Maju releases through its canonical Maju-hosted Git repository. GitHub
(`heap-cider/maju`) runs the public builds and hosts downloads; it is not the
writable development source. Use the project-local
[release-maju skill](.agents/skills/release-maju/SKILL.md) for this workflow.

## Release bundle

One immutable `vX.Y.Z` tag identifies the entire release:

| Artifact | Published location |
|----------|--------------------|
| Windows x86_64 installer and updater signature | `Maju-X.Y.Z-windows-x86_64-setup.exe` and `.exe.sig` on the GitHub release |
| Signed Android APK | `Maju-X.Y.Z-android.apk` on the GitHub release |
| Linux x86_64 relay archive | `maju-relay-X.Y.Z-linux-x86_64.tar.gz` on the GitHub release |
| Linux amd64/arm64 relay image | `ghcr.io/heap-cider/maju:X.Y.Z` |
| Windows updater metadata | `latest.json` on the release and `maju-desktop-latest` |

`SHA256SUMS` accompanies the release downloads. The Windows installer is not
Authenticode-signed; its updater payload has a Tauri signature. Android's
`versionCode` is derived from the release version so an upgrade preserves app
data. macOS and iOS are not public Maju release targets.

## Prepare and publish

1. Activate the repository's Hermit environment. Verify that `origin` points
   to the canonical Maju-hosted repository and fetch its current `main`.
2. Create a separate release worktree on `version-bump/X.Y.Z` from that exact
   `origin/main`. Run `just bump-desktop-version X.Y.Z` and update
   `CHANGELOG.md` from the preceding stable release.
3. Run the version-manifest check, release contract tests, and `cargo deny
   check`, then the affected quality gates. Commit with `git commit -s`.
4. Push the branch to Maju, open the PR with `maju pr open`, verify its head
   and base, and complete the normal Maju merge.
5. Record the verified merged commit. Require GitHub `main` to be its ancestor,
   then publish that exact commit with a normal fast-forward push to
   `https://github.com/heap-cider/maju.git`. No persistent GitHub remote is needed.
6. Wait for the GitHub `CI` push run on that exact commit to succeed. Only then
   create a signed annotated `vX.Y.Z` tag. Push it to Maju first, then push the
   same tag object to the GitHub URL.
7. Wait for `Release` and `Docker image`. Verify every artifact above, the
   multi-architecture image, and the Windows updater metadata.

The desktop manifests are the bundle's version source. The bump recipe keeps
them and their lockfiles consistent. Android and relay publication derive the
public version from the immutable tag; internal crate versions are separate.

Do not move an existing release tag or overwrite divergent GitHub history.
Public publication does not deploy a running relay. Deploy separately only
when the operator requested it, preserving its signing key and persistent data.

## Publication prerequisites

The operator needs access to the canonical Maju repository and permission to
publish to `heap-cider/maju`. GitHub CLI commands must explicitly include
`--repo heap-cider/maju`.

The release workflow uses these GitHub Actions secrets:

- `MAJU_UPDATER_PUBLIC_KEY`, `TAURI_SIGNING_PRIVATE_KEY`, and
  `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` for the Windows updater.
- `MAJU_ANDROID_UPLOAD_KEYSTORE_B64`, `MAJU_ANDROID_UPLOAD_KEYSTORE_PASSWORD`,
  `MAJU_ANDROID_UPLOAD_KEY_ALIAS`, and `MAJU_ANDROID_UPLOAD_KEY_PASSWORD`
  for Android signing.

The GHCR package must be public so relay operators can pull without registry
credentials. Stable releases also update the image's stable aliases; matching
`debug-` image tags retain profiling information.

## Retry and troubleshooting

A failed publisher is retried at the existing immutable release tag through
its workflow's manual dispatch. First verify that Maju, GitHub, and the release
commit still agree. Never recreate or move the tag to retry a build.

If the Windows updater reports no update, check `maju-desktop-latest/latest.json`
and its `windows-x86_64` entry, then verify that its URL and signature match the
installer from the same version. Other desktop platform entries are not part
of the Maju updater contract.

Upstream internal mobile candidates, Apple signing workflows, and private
Buildkite release instructions are not the Maju public release workflow.
