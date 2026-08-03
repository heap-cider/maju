import assert from "node:assert/strict";
import test from "node:test";

import {
  retryInboxContextRequest,
  shouldReportInboxContextLoadError,
} from "./inboxContextLoad.ts";

test("Inbox context retries a transient startup failure", async () => {
  let attempts = 0;
  const delays = [];
  const result = await retryInboxContextRequest(
    async () => {
      attempts += 1;
      if (attempts < 3) throw new Error("relay is still connecting");
      return "loaded";
    },
    async (delayMs) => {
      delays.push(delayMs);
    },
  );

  assert.equal(result, "loaded");
  assert.equal(attempts, 3);
  assert.deepEqual(delays, [250, 750]);
});

test("a partial context response does not become a persistent error banner", () => {
  assert.equal(
    shouldReportInboxContextLoadError({
      ancestorFailed: false,
      descendantFailed: true,
      loadedContextCount: 1,
    }),
    false,
  );
  assert.equal(
    shouldReportInboxContextLoadError({
      ancestorFailed: true,
      descendantFailed: true,
      loadedContextCount: 0,
    }),
    true,
  );
});
