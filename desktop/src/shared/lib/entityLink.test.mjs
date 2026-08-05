import assert from "node:assert/strict";
import test from "node:test";

import {
  buildIssueLink,
  buildPullRequestLink,
  buildRepoLink,
  entityLinkProjectRouteId,
  isEntityLink,
  parseEntityLink,
} from "./entityLink.ts";

const OWNER =
  "71d67180ba17e749ee825fc8819c9c6ee7003617e1c126504f9b658070ab9224";
const EVENT_ID =
  "c3b589fa5713ba25bad6dc095e2de00a4ac8f50050fdea00fc6444e603be1dd1";

// Golden format strings — must match the Rust builder in
// crates/maju-cli/src/links.rs (`golden_format_matches_desktop` test).
test("builders emit the canonical cross-language link format", () => {
  assert.equal(
    buildPullRequestLink({ id: EVENT_ID, owner: OWNER, dtag: "maju-world" }),
    `maju://pr?id=${EVENT_ID}&owner=${OWNER}&d=maju-world`,
  );
  assert.equal(
    buildIssueLink({ id: EVENT_ID, owner: OWNER, dtag: "maju-world" }),
    `maju://issue?id=${EVENT_ID}&owner=${OWNER}&d=maju-world`,
  );
  assert.equal(
    buildRepoLink({ owner: OWNER, dtag: "maju-world" }),
    `maju://repo?owner=${OWNER}&d=maju-world`,
  );
});

test("builders reject invalid identifiers", () => {
  assert.throws(() =>
    buildRepoLink({ owner: "not-a-pubkey", dtag: "maju-world" }),
  );
  assert.throws(() => buildRepoLink({ owner: OWNER, dtag: ".hidden" }));
  assert.throws(() => buildRepoLink({ owner: OWNER, dtag: "a..b" }));
  assert.throws(() =>
    buildPullRequestLink({ id: "short", owner: OWNER, dtag: "maju-world" }),
  );
});

test("parseEntityLink round-trips built links", () => {
  const link = buildPullRequestLink({
    id: EVENT_ID,
    owner: OWNER,
    dtag: "maju-world",
  });
  assert.deepEqual(parseEntityLink(link), {
    ok: true,
    value: { type: "pr", id: EVENT_ID, owner: OWNER, dtag: "maju-world" },
  });

  const repoLink = buildRepoLink({ owner: OWNER, dtag: "maju-world" });
  assert.deepEqual(parseEntityLink(repoLink), {
    ok: true,
    value: { type: "repo", owner: OWNER, dtag: "maju-world" },
  });
});

test("parseEntityLink lowercase-normalizes hex identifiers", () => {
  const parsed = parseEntityLink(
    `maju://issue?id=${EVENT_ID.toUpperCase()}&owner=${OWNER.toUpperCase()}&d=maju-world`,
  );
  assert.deepEqual(parsed, {
    ok: true,
    value: { type: "issue", id: EVENT_ID, owner: OWNER, dtag: "maju-world" },
  });
});

test("parseEntityLink rejects malformed links", () => {
  const cases = [
    ["not a url at all", "invalid-url"],
    [`https://pr?id=${EVENT_ID}&owner=${OWNER}&d=repo`, "wrong-scheme"],
    [`maju://message?channel=x&id=${EVENT_ID}`, "wrong-host"],
    [`maju://pr?id=${EVENT_ID}&owner=nope&d=repo`, "invalid-owner"],
    [`maju://pr?id=${EVENT_ID}&owner=${OWNER}&d=.hidden`, "invalid-dtag"],
    [`maju://pr?id=${EVENT_ID}&owner=${OWNER}`, "invalid-dtag"],
    [`maju://pr?owner=${OWNER}&d=repo`, "invalid-id"],
    [`maju://issue?id=short&owner=${OWNER}&d=repo`, "invalid-id"],
  ];
  for (const [href, reason] of cases) {
    assert.deepEqual(parseEntityLink(href), { ok: false, reason }, href);
  }
});

test("isEntityLink matches entity hosts and excludes message links", () => {
  assert.equal(isEntityLink(`maju://pr?id=${EVENT_ID}`), true);
  assert.equal(isEntityLink(`maju://issue?id=${EVENT_ID}`), true);
  assert.equal(isEntityLink(`maju://repo?owner=${OWNER}`), true);
  assert.equal(isEntityLink("maju://message?channel=x&id=y"), false);
  assert.equal(isEntityLink("https://github.com/heap-cider/maju"), false);
  assert.equal(isEntityLink(null), false);
});

test("entityLinkProjectRouteId emits the canonical 30617 coordinate route id", () => {
  const parsed = parseEntityLink(
    buildRepoLink({ owner: OWNER, dtag: "maju-world" }),
  );
  assert.ok(parsed.ok);
  assert.equal(
    entityLinkProjectRouteId(parsed.value),
    `30617:${OWNER}:maju-world`,
  );
});

test("parseEntityLink rejects noncanonical extras", () => {
  // Unexpected path segments — reserved for future versioning.
  assert.deepEqual(
    parseEntityLink(
      `maju://pr/ignored?id=${EVENT_ID}&owner=${OWNER}&d=maju-world`,
    ),
    { ok: false, reason: "unexpected-path" },
  );
  // Fragment — not part of the canonical format.
  assert.deepEqual(
    parseEntityLink(`maju://repo?owner=${OWNER}&d=maju-world#section`),
    { ok: false, reason: "unexpected-fragment" },
  );
  // Unknown query parameter — reject to preserve forward-compat posture.
  assert.deepEqual(
    parseEntityLink(
      `maju://repo?owner=${OWNER}&d=maju-world&relay=wss%3A%2F%2Frelay.example`,
    ),
    { ok: false, reason: "unknown-param" },
  );
  // Duplicate required parameter — reject.
  assert.deepEqual(
    parseEntityLink(`maju://repo?owner=${OWNER}&d=maju-world&owner=${OWNER}`),
    { ok: false, reason: "duplicate-param" },
  );
});
