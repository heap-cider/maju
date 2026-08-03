import assert from "node:assert/strict";
import test from "node:test";

import { canModerateCommunityContent } from "./contentModeration.ts";

test("community owners and admins may moderate channel content", () => {
  assert.equal(canModerateCommunityContent("owner"), true);
  assert.equal(canModerateCommunityContent("admin"), true);
});

test("ordinary roles and missing membership may not moderate content", () => {
  for (const role of ["member", "guest", "bot", null, undefined]) {
    assert.equal(canModerateCommunityContent(role), false);
  }
});
