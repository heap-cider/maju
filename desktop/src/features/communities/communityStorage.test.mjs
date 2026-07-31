import assert from "node:assert/strict";
import test from "node:test";

import {
  clearCommunityStorage,
  initFirstCommunity,
  migrateLegacyCommunityStorage,
  shouldAutoConnectDefaultRelay,
} from "./communityStorage.ts";

function createMemoryStorage(initial = {}) {
  const values = new Map(Object.entries(initial));
  return {
    values,
    getItem: (key) => values.get(key) ?? null,
    setItem: (key, value) => values.set(key, String(value)),
    removeItem: (key) => values.delete(key),
    clear: () => values.clear(),
    key: (index) => Array.from(values.keys())[index] ?? null,
    get length() {
      return values.size;
    },
  };
}

test("migrateLegacyCommunityStorage promotes current Maju workspace state", () => {
  const storage = createMemoryStorage({
    "maju-workspaces": '[{"id":"current"}]',
    "maju-active-workspace-id": "current",
  });

  migrateLegacyCommunityStorage(storage);

  assert.equal(storage.getItem("maju-communities"), '[{"id":"current"}]');
  assert.equal(storage.getItem("maju-active-community-id"), "current");
});

test("migrateLegacyCommunityStorage does not overwrite new community state", () => {
  const storage = createMemoryStorage({
    "maju-communities": '[{"id":"new"}]',
    "maju-active-community-id": "new",
    "maju-workspaces": '[{"id":"old"}]',
    "maju-active-workspace-id": "old",
  });

  migrateLegacyCommunityStorage(storage);

  assert.equal(storage.getItem("maju-communities"), '[{"id":"new"}]');
  assert.equal(storage.getItem("maju-active-community-id"), "new");
});

test("signed-build relay defaults auto-connect during first-run onboarding", () => {
  assert.equal(
    shouldAutoConnectDefaultRelay("wss://maju.block.builderlab.xyz"),
    true,
  );
  assert.equal(shouldAutoConnectDefaultRelay("ws://localhost:3000"), false);
  assert.equal(shouldAutoConnectDefaultRelay("ws://127.0.0.1:3000"), false);
  assert.equal(shouldAutoConnectDefaultRelay("ws://[::1]:3000"), false);
  assert.equal(shouldAutoConnectDefaultRelay("ws://0.0.0.0:3000"), false);
  assert.equal(shouldAutoConnectDefaultRelay("http://localhost:3000"), false);
  assert.equal(
    shouldAutoConnectDefaultRelay("https://relay.example.com"),
    false,
  );
  assert.equal(shouldAutoConnectDefaultRelay("relay.example.com"), false);
  assert.equal(shouldAutoConnectDefaultRelay("not a valid relay"), false);
});

test("failed first-community write preserves existing community data", () => {
  const storage = createMemoryStorage({
    "maju-communities": '[{"id":"existing"}]',
    "maju-workspaces": '[{"id":"legacy"}]',
    "maju-active-workspace-id": "legacy",
  });
  storage.setItem = (key, value) => {
    if (key === "maju-communities") {
      throw new Error("QuotaExceededError");
    }
    storage.values.set(key, String(value));
  };
  globalThis.localStorage = storage;
  globalThis.window = { localStorage: storage };

  assert.equal(initFirstCommunity("wss://relay.example.com", "pubkey"), null);
  assert.equal(storage.getItem("maju-communities"), '[{"id":"existing"}]');
  assert.equal(storage.getItem("maju-active-community-id"), null);
  assert.equal(storage.getItem("maju-workspaces"), '[{"id":"legacy"}]');
  assert.equal(storage.getItem("maju-active-workspace-id"), "legacy");
});

test("clearCommunityStorage removes new and legacy state", () => {
  const storage = createMemoryStorage({
    "maju-communities": "new",
    "maju-active-community-id": "new",
    "maju-workspaces": "old",
    "maju-active-workspace-id": "old",
  });

  clearCommunityStorage(storage);
  migrateLegacyCommunityStorage(storage);

  assert.equal(storage.length, 0);
});
