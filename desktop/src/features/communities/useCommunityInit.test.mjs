import assert from "node:assert/strict";
import { after, before, test } from "node:test";

import { JSDOM } from "jsdom";

const dom = new JSDOM("<!doctype html><html><body></body></html>", {
  url: "http://localhost",
});

before(() => {
  Object.assign(globalThis, {
    document: dom.window.document,
    HTMLElement: dom.window.HTMLElement,
    IS_REACT_ACT_ENVIRONMENT: true,
    localStorage: dom.window.localStorage,
    window: dom.window,
  });
});

after(() => dom.window.close());

const OWNER_PUBKEY = "ab".repeat(32);
const COMMUNITY_A = {
  id: "community-a",
  name: "A",
  relayUrl: "wss://a.example",
  addedAt: "2026-01-01T00:00:00Z",
};
const COMMUNITY_B = {
  id: "community-b",
  name: "B",
  relayUrl: "wss://b.example",
  addedAt: "2026-01-01T00:00:00Z",
};

test("a cancelled delayed switch never applies after the latest community", async () => {
  const { act, cleanup, renderHook, waitFor } = await import(
    "@testing-library/react"
  );
  const { useCommunityInit } = await import("./useCommunityInit.ts");

  const appliedRelays = [];
  let generation = 0;
  let deferNextDrain = false;
  let drainStarted = 0;
  let releaseDelayedDrain;

  dom.window.__TAURI_INTERNALS__ = {
    invoke: (command, payload) => {
      if (command === "get_identity") {
        return Promise.resolve({
          pubkey: OWNER_PUBKEY,
          display_name: "Owner",
        });
      }
      if (command === "clear_pending_navigation_deep_links") {
        if (deferNextDrain) {
          deferNextDrain = false;
          drainStarted += 1;
          return new Promise((resolve) => {
            releaseDelayedDrain = resolve;
          });
        }
        return Promise.resolve();
      }
      if (command === "apply_workspace") {
        const relayUrl = payload.relayUrl;
        appliedRelays.push(relayUrl);
        return Promise.resolve({
          scopeId: `scope:${relayUrl}`,
          relayUrl,
          ownerPubkey: OWNER_PUBKEY,
          generation: ++generation,
        });
      }
      return Promise.reject(new Error(`unmocked Tauri command: ${command}`));
    },
    transformCallback: () => Math.random(),
  };

  const rendered = renderHook(
    ({ community, communityKey }) =>
      useCommunityInit(community, communityKey, false),
    {
      initialProps: {
        community: COMMUNITY_A,
        communityKey: "community-a:initial",
      },
    },
  );

  try {
    await waitFor(() => assert.equal(rendered.result.current.isReady, true));
    assert.deepEqual(appliedRelays, [COMMUNITY_A.relayUrl]);

    deferNextDrain = true;
    rendered.rerender({
      community: COMMUNITY_B,
      communityKey: "community-b:slow",
    });
    await waitFor(() => assert.equal(drainStarted, 1));

    rendered.rerender({
      community: COMMUNITY_A,
      communityKey: "community-a:latest",
    });
    await waitFor(() => {
      assert.equal(rendered.result.current.isReady, true);
      assert.equal(rendered.result.current.appliedKey, "community-a:latest");
    });

    await act(async () => {
      releaseDelayedDrain();
      await Promise.resolve();
    });

    assert.deepEqual(
      appliedRelays,
      [COMMUNITY_A.relayUrl, COMMUNITY_A.relayUrl],
      "the cancelled B effect must not reach apply_workspace",
    );
  } finally {
    rendered.unmount();
    cleanup();
    delete dom.window.__TAURI_INTERNALS__;
  }
});
