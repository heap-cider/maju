import assert from "node:assert/strict";
import test from "node:test";

import {
  extractSupportedLinkPreviews,
  isSupportedLinkAutolinkLabel,
  parseSupportedLinkPreview,
} from "./linkPreview.ts";

test("parseSupportedLinkPreview parses GitHub pull request URLs", () => {
  assert.deepEqual(
    parseSupportedLinkPreview("https://github.com/block/sprout/pull/1234"),
    {
      kind: "github-pull-request",
      href: "https://github.com/block/sprout/pull/1234",
      provider: "GitHub",
      title: "block/sprout #1234",
      typeLabel: "PR",
    },
  );
});

test("parseSupportedLinkPreview parses GitHub repository URLs", () => {
  assert.deepEqual(
    parseSupportedLinkPreview("https://github.com/block/sprout"),
    {
      kind: "github-repository",
      href: "https://github.com/block/sprout",
      provider: "GitHub",
      title: "block/sprout",
      typeLabel: "repo",
    },
  );
});

test("parseSupportedLinkPreview trims markdown punctuation around GitHub URLs", () => {
  assert.deepEqual(
    parseSupportedLinkPreview("https://github.com/block/sprout/pull/1234)."),
    {
      kind: "github-pull-request",
      href: "https://github.com/block/sprout/pull/1234",
      provider: "GitHub",
      title: "block/sprout #1234",
      typeLabel: "PR",
    },
  );
});

test("parseSupportedLinkPreview ignores unsupported GitHub URLs", () => {
  assert.equal(
    parseSupportedLinkPreview("https://github.com/block/sprout/tree/main"),
    null,
  );
});

const MAJU_OWNER =
  "71d67180ba17e749ee825fc8819c9c6ee7003617e1c126504f9b658070ab9224";

test("parseSupportedLinkPreview parses Maju relay git clone URLs", () => {
  // Must pass the active relay origin for host validation.
  assert.deepEqual(
    parseSupportedLinkPreview(
      `https://maju.block.builderlab.xyz/git/${MAJU_OWNER}/maju-world-galaxy`,
      "https://maju.block.builderlab.xyz",
    ),
    {
      kind: "maju-repository",
      href: `maju://repo?owner=${MAJU_OWNER}&d=maju-world-galaxy`,
      provider: "Maju",
      title: "maju-world-galaxy",
      typeLabel: "repo",
    },
  );
  // Same URL without a matching origin stays external.
  assert.equal(
    parseSupportedLinkPreview(
      `https://maju.block.builderlab.xyz/git/${MAJU_OWNER}/maju-world-galaxy`,
    ),
    null,
  );
});

test("parseSupportedLinkPreview strips .git suffix from clone URLs", () => {
  assert.deepEqual(
    parseSupportedLinkPreview(
      `http://localhost:3000/git/${MAJU_OWNER}/maju-world.git`,
      "http://localhost:3000",
    ),
    {
      kind: "maju-repository",
      href: `maju://repo?owner=${MAJU_OWNER}&d=maju-world`,
      provider: "Maju",
      title: "maju-world",
      typeLabel: "repo",
    },
  );
});

test("parseSupportedLinkPreview rejects malformed Maju git URLs", () => {
  for (const href of [
    // Owner segment must be a 64-char lowercase hex pubkey.
    "https://relay.example/git/not-a-pubkey/repo",
    `https://relay.example/git/${MAJU_OWNER.toUpperCase()}/repo`,
    `https://relay.example/git/${MAJU_OWNER.slice(0, 32)}/repo`,
    // Missing or invalid repo segment.
    `https://relay.example/git/${MAJU_OWNER}`,
    `https://relay.example/git/${MAJU_OWNER}/.hidden`,
    // Deeper transport paths are not repo links.
    `https://relay.example/git/${MAJU_OWNER}/repo/info/refs`,
  ]) {
    // Even with a matching origin, structural issues return null.
    assert.equal(
      parseSupportedLinkPreview(href, "https://relay.example"),
      null,
      href,
    );
  }
});

test("parseSupportedLinkPreview rejects clone URLs from non-relay hosts", () => {
  // Correct path shape but origin does not match the active relay.
  assert.equal(
    parseSupportedLinkPreview(
      `https://evil.example/git/${MAJU_OWNER}/my-repo`,
      "https://maju.block.builderlab.xyz",
    ),
    null,
  );
  // github.com sharing the path shape must never become a Maju repo card.
  assert.equal(
    parseSupportedLinkPreview(
      `https://github.com/git/${MAJU_OWNER}/my-repo`,
      "https://maju.block.builderlab.xyz",
    ),
    null,
  );
  // No relay origin provided — stays external.
  assert.equal(
    parseSupportedLinkPreview(
      `https://maju.block.builderlab.xyz/git/${MAJU_OWNER}/maju-world`,
      null,
    ),
    null,
  );
});

const MAJU_EVENT_ID =
  "c3b589fa5713ba25bad6dc095e2de00a4ac8f50050fdea00fc6444e603be1dd1";

test("parseSupportedLinkPreview parses maju:// PR and issue deep links", () => {
  assert.deepEqual(
    parseSupportedLinkPreview(
      `maju://pr?id=${MAJU_EVENT_ID}&owner=${MAJU_OWNER}&d=maju-world`,
    ),
    {
      kind: "maju-pull-request",
      href: `maju://pr?id=${MAJU_EVENT_ID}&owner=${MAJU_OWNER}&d=maju-world`,
      provider: "Maju",
      title: "maju-world #c3b589fa",
      typeLabel: "PR",
    },
  );
  assert.deepEqual(
    parseSupportedLinkPreview(
      `maju://issue?id=${MAJU_EVENT_ID}&owner=${MAJU_OWNER}&d=maju-world`,
    )?.typeLabel,
    "issue",
  );
  assert.deepEqual(
    parseSupportedLinkPreview(`maju://repo?owner=${MAJU_OWNER}&d=maju-world`),
    {
      kind: "maju-repository",
      href: `maju://repo?owner=${MAJU_OWNER}&d=maju-world`,
      provider: "Maju",
      title: "maju-world",
      typeLabel: "repo",
    },
  );
});

test("parseSupportedLinkPreview rejects malformed maju:// entity links", () => {
  for (const href of [
    `maju://pr?owner=${MAJU_OWNER}&d=maju-world`,
    `maju://pr?id=short&owner=${MAJU_OWNER}&d=maju-world`,
    `maju://issue?id=${MAJU_EVENT_ID}&owner=nope&d=maju-world`,
    `maju://repo?owner=${MAJU_OWNER}&d=.hidden`,
  ]) {
    assert.equal(parseSupportedLinkPreview(href), null, href);
  }
});

test("extractSupportedLinkPreviews picks up maju:// links in prose", () => {
  assert.deepEqual(
    extractSupportedLinkPreviews(
      `PR is up: maju://pr?id=${MAJU_EVENT_ID}&owner=${MAJU_OWNER}&d=maju-world — review please.`,
    ).map((preview) => [preview.kind, preview.title]),
    [["maju-pull-request", "maju-world #c3b589fa"]],
  );
});

test("extractSupportedLinkPreviews uses markdown labels for maju:// links", () => {
  assert.deepEqual(
    extractSupportedLinkPreviews(
      `[Add header links](maju://pr?id=${MAJU_EVENT_ID}&owner=${MAJU_OWNER}&d=maju-world)`,
    ).map((preview) => preview.title),
    ["Add header links"],
  );
});

test("parseSupportedLinkPreview parses Linear issue URLs", () => {
  assert.deepEqual(
    parseSupportedLinkPreview(
      "https://linear.app/maju/issue/BUG-321/fix-link-previews",
    ),
    {
      kind: "linear-issue",
      href: "https://linear.app/maju/issue/BUG-321/fix-link-previews",
      provider: "Linear",
      title: "BUG-321",
      typeLabel: "issue",
    },
  );
});

test("parseSupportedLinkPreview normalizes Linear issue URL variants", () => {
  assert.deepEqual(
    parseSupportedLinkPreview("linear.app/maju/issue/a-7/fix-link-previews"),
    {
      kind: "linear-issue",
      href: "https://linear.app/maju/issue/a-7/fix-link-previews",
      provider: "Linear",
      title: "A-7",
      typeLabel: "issue",
    },
  );
});

test("parseSupportedLinkPreview parses Google app URLs", () => {
  assert.deepEqual(
    [
      "https://drive.google.com/file/d/abc123/view",
      "https://drive.google.com/drive/folders/folder123",
      "https://docs.google.com/document/d/doc123/edit",
      "https://docs.google.com/spreadsheets/d/sheet123/edit",
      "https://docs.google.com/presentation/d/slides123/edit",
    ].map((href) => parseSupportedLinkPreview(href)?.kind),
    [
      "google-drive-file",
      "google-drive-folder",
      "google-docs-document",
      "google-sheets-spreadsheet",
      "google-slides-presentation",
    ],
  );
});

test("extractSupportedLinkPreviews returns unique supported links in order", () => {
  assert.deepEqual(
    extractSupportedLinkPreviews(
      [
        "See github.com/block/sprout/pull/1",
        "and https://linear.app/maju/issue/BUG-2/fix-preview",
        "then https://github.com/block/sprout/pull/1 again.",
        "plus https://docs.google.com/document/d/doc123/edit",
      ].join(" "),
    ).map((preview) => preview.title),
    ["block/sprout #1", "BUG-2", "Document"],
  );
});

test("extractSupportedLinkPreviews picks up bare Maju clone URLs in prose", () => {
  assert.deepEqual(
    extractSupportedLinkPreviews(
      `master pushed; clone: https://maju.block.builderlab.xyz/git/${MAJU_OWNER}/maju-world-galaxy and review please.`,
      "https://maju.block.builderlab.xyz",
    ),
    [
      {
        kind: "maju-repository",
        href: `maju://repo?owner=${MAJU_OWNER}&d=maju-world-galaxy`,
        provider: "Maju",
        title: "maju-world-galaxy",
        typeLabel: "repo",
      },
    ],
  );
  // Without a relay origin the URL is treated as an ordinary external link.
  assert.deepEqual(
    extractSupportedLinkPreviews(
      `clone: https://maju.block.builderlab.xyz/git/${MAJU_OWNER}/maju-world-galaxy`,
    ),
    [],
  );
});

test("extractSupportedLinkPreviews uses markdown labels for Maju repo links", () => {
  assert.deepEqual(
    extractSupportedLinkPreviews(
      `[Maju World](https://relay.example/git/${MAJU_OWNER}/maju-world-galaxy)`,
      "https://relay.example",
    ).map((preview) => preview.title),
    ["Maju World"],
  );
});

test("extractSupportedLinkPreviews dedupes clone URL variants of one repo", () => {
  assert.deepEqual(
    extractSupportedLinkPreviews(
      [
        `https://relay.example/git/${MAJU_OWNER}/maju-world-galaxy`,
        `https://relay.example/git/${MAJU_OWNER}/maju-world-galaxy.git`,
      ].join(" "),
      "https://relay.example",
    ).map((preview) => preview.href),
    [`maju://repo?owner=${MAJU_OWNER}&d=maju-world-galaxy`],
  );
});

test("clone URLs and maju://repo links for the same repo dedupe to one card", () => {
  assert.deepEqual(
    extractSupportedLinkPreviews(
      [
        `https://relay.example/git/${MAJU_OWNER}/maju-world-galaxy`,
        `maju://repo?owner=${MAJU_OWNER}&d=maju-world-galaxy`,
      ].join(" "),
    ).map((preview) => preview.href),
    [`maju://repo?owner=${MAJU_OWNER}&d=maju-world-galaxy`],
  );
});

test("extractSupportedLinkPreviews handles markdown link serialization", () => {
  assert.deepEqual(
    extractSupportedLinkPreviews(
      "[https://github.com/block/sprout/pull/44](https://github.com/block/sprout/pull/44)",
    ).map((preview) => preview.title),
    ["block/sprout #44"],
  );
});

test("extractSupportedLinkPreviews uses useful markdown labels as titles", () => {
  assert.deepEqual(
    extractSupportedLinkPreviews(
      "[Composer attachment polish](https://docs.google.com/document/d/doc123/edit)",
    ),
    [
      {
        kind: "google-docs-document",
        href: "https://docs.google.com/document/d/doc123/edit",
        provider: "Google Docs",
        title: "Composer attachment polish",
        typeLabel: "document",
      },
    ],
  );
});

test("extractSupportedLinkPreviews includes multiple supported Google links", () => {
  assert.deepEqual(
    extractSupportedLinkPreviews(
      [
        "https://docs.google.com/document/d/doc123/edit",
        "https://docs.google.com/spreadsheets/d/sheet123/edit",
        "https://docs.google.com/presentation/d/slides123/edit",
      ].join(" "),
    ).map((preview) => preview.kind),
    [
      "google-docs-document",
      "google-sheets-spreadsheet",
      "google-slides-presentation",
    ],
  );
});

test("extractSupportedLinkPreviews skips URLs inside inline and fenced code", () => {
  assert.deepEqual(
    extractSupportedLinkPreviews(
      [
        "`https://github.com/block/sprout/pull/1`",
        "```",
        "https://linear.app/maju/issue/BUG-2/fix-preview",
        "```",
        "https://github.com/block/sprout/pull/3",
      ].join("\n"),
    ).map((preview) => preview.title),
    ["block/sprout #3"],
  );
});

test("extractSupportedLinkPreviews skips URLs inside indented code", () => {
  assert.deepEqual(
    extractSupportedLinkPreviews(
      [
        "    https://docs.google.com/document/d/hidden/edit",
        "\tgithub.com/block/sprout/pull/4",
        "https://github.com/block/sprout/pull/5",
      ].join("\n"),
    ).map((preview) => preview.title),
    ["block/sprout #5"],
  );
});

test("extractSupportedLinkPreviews skips markdown image link URLs", () => {
  assert.deepEqual(
    extractSupportedLinkPreviews(
      [
        "![alt](https://docs.google.com/document/d/doc123/edit)",
        "![alt](https://github.com/block/sprout)",
        "[Composer attachment polish](https://docs.google.com/document/d/doc456/edit)",
      ].join("\n"),
    ).map((preview) => preview.title),
    ["Composer attachment polish"],
  );
});

test("extractSupportedLinkPreviews requires bare URL boundaries", () => {
  assert.deepEqual(
    extractSupportedLinkPreviews(
      [
        "https://evil-github.com/block/sprout/pull/1",
        "https://example.com/go/https://docs.google.com/document/d/doc123/edit",
        "(https://github.com/block/sprout/pull/2)",
      ].join(" "),
    ).map((preview) => preview.title),
    ["block/sprout #2"],
  );
});

test("extractSupportedLinkPreviews skips links inside inline spoilers", () => {
  assert.deepEqual(
    extractSupportedLinkPreviews(
      [
        "Keep",
        "||[roadmap](https://docs.google.com/document/d/hidden/edit)||",
        "hidden, but show https://github.com/block/sprout/pull/7",
      ].join(" "),
    ).map((preview) => preview.title),
    ["block/sprout #7"],
  );
});

test("extractSupportedLinkPreviews skips links inside block spoilers", () => {
  assert.deepEqual(
    extractSupportedLinkPreviews(
      [
        "||",
        "",
        "https://linear.app/maju/issue/BUG-99/hidden-spoiler-link",
        "",
        "||",
        "https://github.com/block/sprout/pull/8",
      ].join("\n"),
    ).map((preview) => preview.title),
    ["block/sprout #8"],
  );
});

test("isSupportedLinkAutolinkLabel matches normalized bare URL labels", () => {
  const preview = parseSupportedLinkPreview("github.com/block/sprout/pull/5");
  assert.ok(preview);
  assert.equal(
    isSupportedLinkAutolinkLabel(
      "https://github.com/block/sprout/pull/5",
      preview,
    ),
    true,
  );
  assert.equal(isSupportedLinkAutolinkLabel("review this", preview), false);
});

// ── useResolvedLinkPreviews: behavioral regression pins ──────────────────────
//
// These tests pin the three behaviors that were implemented without tests in
// the initial fix round. They use the exported pure helpers directly so no
// React hook environment is required.

import {
  getLinkPreviewCacheGeneration,
  resetLinkPreviewTitleCache,
  shouldResolveTitle,
} from "./useResolvedLinkPreviews.ts";
import { majuEntityFallbackTitle } from "./linkPreview.ts";

const OWNER_HEX =
  "71d67180ba17e749ee825fc8819c9c6ee7003617e1c126504f9b658070ab9224";
const EVENT_HEX =
  "c3b589fa5713ba25bad6dc095e2de00a4ac8f50050fdea00fc6444e603be1dd1";

function makePrPreview(title) {
  return {
    kind: "maju-pull-request",
    href: `maju://pr?id=${EVENT_HEX}&owner=${OWNER_HEX}&d=maju-world`,
    title,
    provider: "Maju",
    typeLabel: "pr",
  };
}

// 1. Cache epoch: stale promise cannot seed the new generation.
//    `resetLinkPreviewTitleCache` must increment the generation counter so that
//    a promise captured before the reset sees a different generation and skips
//    writing back.
test("resetLinkPreviewTitleCache_incrementsGenerationCounter", () => {
  const before = getLinkPreviewCacheGeneration();
  resetLinkPreviewTitleCache();
  const after = getLinkPreviewCacheGeneration();
  assert.equal(after, before + 1, "each reset must bump the generation by 1");
  resetLinkPreviewTitleCache();
  assert.equal(
    getLinkPreviewCacheGeneration(),
    before + 2,
    "second reset must increment again",
  );
});

// 2. Mismatched a-tag: shouldResolveTitle uses majuEntityFallbackTitle to
//    decide whether to attempt a relay lookup. When the link's href parses to a
//    PR/issue with the expected fallback title, resolution should proceed. When
//    the title has already been set to something else (explicit label or earlier
//    relay result), shouldResolveTitle must return false so the label wins.
test("shouldResolveTitle_fallbackTitle_returnsTrue", () => {
  const parsed = {
    ok: true,
    value: { type: "pr", id: EVENT_HEX, owner: OWNER_HEX, dtag: "maju-world" },
  };
  // Construct the expected fallback title and verify shouldResolveTitle allows lookup.
  const fallback = majuEntityFallbackTitle(parsed.value);
  const preview = makePrPreview(fallback);
  assert.equal(
    shouldResolveTitle(preview),
    true,
    "fallback title should trigger relay lookup",
  );
});

test("shouldResolveTitle_customLabel_returnsFalse_labelMustWin", () => {
  // User has written `[My custom PR title](maju://pr?...)` — the label must
  // win; shouldResolveTitle must return false to skip writing the relay title.
  const preview = makePrPreview("My custom PR title");
  assert.equal(
    shouldResolveTitle(preview),
    false,
    "custom label must suppress relay title lookup (label-must-win invariant)",
  );
});

// 3. Label-rerender: converting a bare link to `[label](link)` changes the
//    preview title away from the fallback — shouldResolveTitle transitions
//    from true to false, so a cached relay title is not applied.
test("shouldResolveTitle_transitionsFromTrueToFalseWhenLabelApplied", () => {
  const parsed = {
    ok: true,
    value: { type: "pr", id: EVENT_HEX, owner: OWNER_HEX, dtag: "maju-world" },
  };
  const fallback = majuEntityFallbackTitle(parsed.value);

  // Before the label: bare link with fallback title — should resolve.
  const barePreview = makePrPreview(fallback);
  assert.equal(
    shouldResolveTitle(barePreview),
    true,
    "bare link should resolve",
  );

  // After the label: same href but title is now the user's label — must NOT resolve.
  const labeledPreview = makePrPreview("My labeled PR");
  assert.equal(
    shouldResolveTitle(labeledPreview),
    false,
    "labeled link must not overwrite label with cached relay title",
  );
});
