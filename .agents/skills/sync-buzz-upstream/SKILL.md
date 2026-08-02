---
name: sync-buzz-upstream
description: Inspect and synchronize a new block/buzz release into the heap-cider/maju fork without directly merging upstream. Use when comparing Buzz release tags, reviewing upstream release changes, classifying Maju conflicts, applying user-approved upstream changes, or updating the last synchronized Buzz version.
---

# Sync Buzz Upstream

Treat Buzz as a read-only source. Preserve Maju's naming, product direction,
Windows support, and repository ownership while reviewing upstream releases.

## Analyze a release

1. Read the `Maju Fork Policy` at the top of `AGENTS.md`.
2. Confirm `origin` points to `heap-cider/maju`, `upstream` fetches from
   `block/buzz`, and the upstream push URL is `DISABLED`.
3. Preserve the user's existing worktree changes. Do not merge, rebase,
   cherry-pick, commit, push, or edit files during analysis.
4. Run the bundled analyzer from the repository root:

   ```bash
   node .agents/skills/sync-buzz-upstream/scripts/compare-upstream.mjs \
     --from desktop-v0.5.2 --to desktop-v0.5.3 --fetch --json
   ```

   Pass the exact upstream tag names. The analyzer accepts any valid Git tag
   name instead of assuming a version prefix or release naming scheme, then
   resolves only `refs/tags/<name>` so branches and arbitrary commits cannot be
   substituted for release tags.

   Omit `--fetch` when both tags already exist locally. Use `--expect-zero` for
   a same-version smoke test.
5. Review every reported change. The analyzer translates `buzz`, `Buzz`, and
   `BUZZ` in upstream paths and text to their Maju equivalents before comparing
   them with the current worktree.
6. Present four groups to the user:
   - `safe-to-apply`: Maju still matches the normalized old Buzz file.
   - `already-applied`: Maju already matches the normalized new Buzz file.
   - `conflict`: Maju changed the same text file or path.
   - `manual-review`: divergent binary or unsupported file transition.
7. Explain the upstream intent, the corresponding Maju files, and the proposed
   choice for every conflict. Wait for explicit user approval before applying
   anything.

## Apply approved changes

1. Apply only the approved items to Maju-native names and paths. Never restore
   Buzz identifiers merely to reduce the diff.
2. Do not send issues, pull requests, patches, or messages to Buzz.
3. If Buzz now fixes a problem Maju fixed earlier, compare both approaches and
   obtain the user's agreement before replacing Maju's implementation.
4. Run the quality gates required by the affected areas in `AGENTS.md`.
5. Re-run the analyzer and inspect remaining conflicts.
6. Update the synchronized Buzz version in `AGENTS.md` only after the approved
   changes and tests are complete.
7. Report applied, skipped, conflicting, and remaining changes separately.

## Analyzer guarantees

`scripts/compare-upstream.mjs` reads Git objects and the worktree. `--fetch`
may add missing upstream tag refs, but the script must leave the worktree status
byte-for-byte unchanged. Treat a nonzero exit or a worktree-integrity failure as
a blocked analysis and do not continue to application.
