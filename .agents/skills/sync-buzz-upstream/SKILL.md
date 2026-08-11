---
name: sync-buzz-upstream
description: Inspect and synchronize a new block/buzz release into Maju without directly merging upstream or requiring a persistent Git remote. Use when comparing Buzz release tags, reviewing upstream release changes, classifying Maju conflicts, applying user-approved upstream changes, or updating the last synchronized Buzz version.
---

# Sync Buzz Upstream

Treat Buzz as a read-only source. Preserve Maju's naming, current product
contract, supported releases, and repository ownership while reviewing upstream
releases.

## Analyze a release

1. Read the `Maju Product Contract` and `Maju Fork Policy` sections at the top
   of `AGENTS.md`, then read `MAJU_PRODUCT_CONTRACT.md` in full. Treat the
   contract as desired product behavior, not proof of implementation.
2. Do not require or create a persistent `upstream` remote. The bundled
   analyzer fetches exact release tags directly from the fixed read-only source
   `https://github.com/block/buzz.git` and removes its temporary refs before it
   exits. Never push, open issues, or open pull requests against that source.
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
   substituted for release tags. It normally requires direct tag ancestry. If
   Buzz tagged a release-branch commit and later squash-merged that release
   back to main, the analyzer also accepts an ancestor of the newer tag whose
   complete Git tree is byte-identical to the older tag. It reports this as
   `tree-equivalent-ancestor`. If an unrelated change lands between the reviewed
   tag and that release commit, it accepts only an exact Git object-delta
   replay on a continuation of the reviewed tag's parent and reports
   `patch-equivalent-ancestor`. The original tag remains the comparison base so
   the interleaved change is included. Unrelated or merely similar histories
   still fail closed.

   Use `--fetch` for the normal path so both tags are resolved from the fixed
   official source without trusting local remote configuration. Omit it only
   when intentionally testing already-present local tags. Use `--expect-zero`
   for a same-version smoke test.
5. Review every reported change. The analyzer translates `buzz`, `Buzz`, and
   `BUZZ` in upstream paths and text to their Maju equivalents before comparing
   them with the current worktree. Independently check each user-visible change
   against `MAJU_PRODUCT_CONTRACT.md`; a clean text comparison does not make a
   product-contract conflict safe.
   Judge source synchronization separately from release support. Maju not
   shipping a macOS, iOS, or Linux desktop build does not by itself make shared
   upstream code a conflict. Apply compatible shared code while continuing to
   exclude unsupported release artifacts and pipelines.
6. Present four groups to the user:
   - `safe-to-apply`: Maju still matches the normalized old Buzz file and the
     upstream behavior does not contradict the Maju product contract.
   - `already-applied`: Maju already matches the normalized new Buzz file.
   - `conflict`: Maju changed the same text file or path, or the upstream
     behavior contradicts or would make stale a current Maju product decision.
   - `manual-review`: divergent binary or unsupported file transition.
7. Keep the report simple. Summarize changes that need no decision, then explain
   only actual conflicts and judgment calls with four short points: user effect,
   Buzz's change, Maju's current difference, and the recommended choice. Name
   the relevant product-contract heading or state that none applies. When Buzz
   fixes the same problem at its source and no Maju product decision conflicts,
   recommend the Buzz fix instead of keeping a Maju-only workaround. Wait for
   explicit user approval before applying anything.
8. Before calling a runtime problem Maju-specific, compare the exact Buzz tag
   with the normalized Maju code and reproduce or trace both paths. Distinguish
   an upstream bug, an agent/tool permission policy, and a real Maju fork
   conflict instead of guessing from one observed failure.

## Apply approved changes

1. Re-read `MAJU_PRODUCT_CONTRACT.md`, then apply only the approved items to
   Maju-native names and paths. Never restore Buzz identifiers merely to reduce
   the diff.
2. Do not send issues, pull requests, patches, or messages to Buzz.
3. If Buzz now fixes a problem Maju fixed earlier, compare both approaches. Use
   the approved upstream root fix when it preserves the product contract; keep
   the Maju implementation only when there is a concrete Maju-specific reason.
4. Update `MAJU_PRODUCT_CONTRACT.md` only when the user explicitly approved a
   change to a Maju product decision. Do not rewrite the contract merely to make
   an upstream change appear compatible. Keep only the resulting current
   decision, without adding synchronization history or implementation details.
5. Run the quality gates required by the affected areas in `AGENTS.md`.
6. Re-run the analyzer, inspect remaining conflicts, and audit the resulting
   behavior against every relevant product-contract heading.
7. Update the synchronized Buzz version in `AGENTS.md` only after the approved
   changes, contract audit, and tests are complete.
8. Report applied, skipped, conflicting, and remaining changes separately,
   including any approved product-contract update.

## Analyzer guarantees

`scripts/compare-upstream.mjs` reads Git objects and the worktree. `--fetch`
contacts only the fixed Buzz URL, creates no Git remote, and removes its
temporary refs before exit. The script must leave the worktree status
byte-for-byte unchanged. Treat a nonzero exit or a worktree-integrity failure
as a blocked analysis and do not continue to application.
