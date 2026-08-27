---
name: sync-buzz-upstream
description: Inspect and synchronize a new block/buzz release into Maju without directly merging upstream or requiring a persistent Git remote. Use when comparing Buzz release tags, reviewing upstream release changes, classifying Maju conflicts, applying user-approved upstream changes, or updating the last synchronized Buzz version.
---

# Sync Buzz Upstream

Treat Buzz as Maju's read-only product default. Preserve only fork normalization
and the narrow differences listed in `MAJU_PRODUCT_CONTRACT.md`. A difference in
current Maju code, tests, documentation, or history is not a product decision by
itself.

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
   them with the current worktree. Classify merge state separately from product
   state:

   - Merge: `clean`, `text-overlap`, `already-applied`, or `binary-review`.
   - Product: `upstream-default`, `fork-normalization`, `contract-delta`, or
     `new-decision-required`.

   A text overlap is a merge task, not evidence of a product conflict. Use this
   order for product judgment:

   1. Preserve the exact behavior required by a contract delta.
   2. Translate names, packages, repositories, distribution, and supported
      artifacts as fork normalization.
   3. Adopt every other compatible Buzz change, including bug fixes, security
      fixes, performance work, refactors, and UX changes.
   4. Ask the user only when Buzz creates a new choice the contract cannot
      answer.

   Judge source synchronization separately from release support. Maju not
   shipping a macOS, iOS, or Linux desktop build does not by itself make shared
   upstream code a conflict. Apply compatible shared code while continuing to
   exclude unsupported release artifacts and pipelines.
6. Start the report with `New product decisions required: N`. Summarize
   user-visible changes, contract deltas, and real decisions in the main report.
   Put merge counts and text overlaps in a short appendix.
7. Do not treat an analysis request as permission to edit. When application is
   already authorized, pause only for `new-decision-required`; continue without
   another approval when `N = 0`.
8. Before calling a runtime problem Maju-specific, compare the exact Buzz tag
   with the normalized Maju code and reproduce or trace both paths. Distinguish
   an upstream bug, an agent/tool permission policy, and a real Maju fork
   conflict instead of guessing from one observed failure.

## Apply approved changes

1. Re-read `MAJU_PRODUCT_CONTRACT.md`, apply the compatible Buzz delta by
   default, and preserve only contract deltas and fork normalization. Never
   restore Buzz identifiers merely to reduce the diff.
2. Do not send issues, pull requests, patches, or messages to Buzz.
3. If Buzz now fixes a problem Maju fixed earlier, use the upstream root fix
   unless an exact contract delta requires the Maju implementation.
4. Update `MAJU_PRODUCT_CONTRACT.md` only for an explicit owner decision. Write
   the smallest user-visible outcome and add no inferred meaning.
5. Run the quality gates required by the affected areas in `AGENTS.md`.
6. Re-run the analyzer and audit every remaining Maju-only behavior. Each must
   map to fork normalization, a contract delta, an unsupported release artifact,
   or a documented non-applicable upstream change.
7. Update the synchronized Buzz version in `AGENTS.md` only after the approved
   changes, contract audit, and tests are complete.
8. Hand release work the target Buzz tag, preserved contract deltas, excluded
   artifacts, exact commit, and test results. Report only the analysis,
   PR/blocker, and verified completion milestones unless a real failure needs
   attention.

## Analyzer guarantees

`scripts/compare-upstream.mjs` reads Git objects and the worktree. `--fetch`
contacts only the fixed Buzz URL, creates no Git remote, and removes its
temporary refs before exit. The script must leave the worktree status
byte-for-byte unchanged. Treat a nonzero exit or a worktree-integrity failure
as a blocked analysis and do not continue to application.
