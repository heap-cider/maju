---
name: release-maju
description: Prepare and publish a public Maju release from the canonical Maju-hosted Git repository to heap-cider/maju without a persistent GitHub remote. Use when creating a Maju version PR, publishing a vX.Y.Z release, running GitHub release builds, checking Windows/Android/Linux artifacts or GHCR publication, or retrying a failed public release.
---

# Release Maju

Keep Maju Git canonical. Use GitHub only to run the existing public build and
publication workflows. Never require or create a persistent GitHub remote.

## Safety rules

- Read the Maju Product Contract and Fork Policy in `AGENTS.md`, then read
  `MAJU_PRODUCT_CONTRACT.md` and the artifact details in `RELEASING.md`.
- Activate the repository Hermit environment before Git, hooks, or release
  commands.
- Require a clean worktree and an exact, current `origin/main`. Confirm
  `origin` is the Maju-hosted repository announced by the active project. Stop
  if `origin` is `github.com/heap-cider/maju` or cannot be tied to that project.
- Address GitHub explicitly as `https://github.com/heap-cider/maju.git` for Git
  and `--repo heap-cider/maju` for `gh`. Do not add or depend on a GitHub remote.
- Never force-push, move an existing release tag, manufacture a tag before its
  release PR is merged, or overwrite divergent GitHub history. Stop and report
  the two commit IDs when either `main` or the tag differs.
- Public release does not authorize deployment to a running relay. Deploy only
  when the user separately asks for it.

## Prepare the release in Maju

1. Resolve the requested semantic version. Use the next patch only when the
   user did not specify a version.
2. Create a release worktree and `version-bump/<version>` branch from the exact
   fetched `origin/main`. Do not prepare releases on the default worktree.
3. Run `just bump-desktop-version <version>`, update `CHANGELOG.md` from the
   previous stable `v*` tag through the base commit, and keep the Windows,
   Android, Linux relay, and GHCR bundle on the same version.
4. Run the relevant full quality gates, including the release contract tests.
5. Commit with signoff, push the branch only to Maju `origin`, and open a Maju
   pull request with `maju pr open`. Include the repository owner, repository
   id, canonical Maju clone URL, branch tip, merge base, branch name, and the
   originating channel when one exists.
6. Report the Maju PR link and stop. Do not publish or tag until that PR is
   actually merged.

## Publish the merged release

1. Fetch the exact current Maju `origin/main` and confirm the release commit is
   present there, all version manifests equal `<version>`, and the worktree is
   clean. Record the immutable release commit SHA.
2. Check both the Maju origin and GitHub URL for `refs/tags/v<version>`. If the
   tag exists anywhere, require it to resolve to the recorded commit; otherwise
   stop. Create one signed annotated tag on that commit and push it to Maju
   `origin` first.
3. Read GitHub `main` directly with `git ls-remote`. Fetch it by URL when it
   exists and require it to be an ancestor of the Maju release commit. Push the
   exact Maju `main` commit to GitHub with a normal fast-forward push by URL.
4. Find the GitHub `CI` push run for the recorded commit and wait for success.
   Do not publish the tag after a missing, cancelled, or failed CI run.
5. Push the exact `v<version>` tag to the GitHub URL without changing it. The
   existing `release.yml` and `docker.yml` tag triggers remain the publishers.
6. Wait for both workflows. Verify the GitHub `v<version>` release contains the
   Windows installer, Android APK, and Linux relay archive; verify the matching
   public `ghcr.io/heap-cider/maju:<version>` image and the rolling desktop
   updater metadata.
7. Report the Maju commit and tag, GitHub release URL, workflow results, and
   published artifact set. Report deployment separately only if requested.

## Retry a failed publisher

Never recreate or move the tag. Confirm the Maju tag, GitHub tag, and release
commit still match, then use the existing workflow's manual retry at that exact
GitHub tag. Re-verify the complete artifact set after the retry.
