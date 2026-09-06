import assert from "node:assert/strict";
import test from "node:test";

import {
  formatMessageSendError,
  getErrorMessage,
  mergeMentionRecipients,
  shouldStartMentionedAgentHere,
  mentionRevalidationOptions,
} from "./useMentionSendFlow.helpers.ts";

const device = (overrides = {}) => ({
  deviceId: "device",
  sessionId: "session",
  name: "PC",
  platform: "windows",
  appVersion: "0.2.1",
  lastSeen: 1,
  online: true,
  current: false,
  activeAgents: [],
  standbyAgents: [],
  ...overrides,
});

test("formatMessageSendError preserves the publication failure", () => {
  assert.equal(
    formatMessageSendError(new Error("relay rejected voice note")),
    "Message failed to send: relay rejected voice note",
  );
});

test("getErrorMessage preserves Tauri string errors", () => {
  assert.equal(
    getErrorMessage(
      "relay returned 415 Unsupported Media Type",
      "Unknown error",
    ),
    "relay returned 415 Unsupported Media Type",
  );
  assert.equal(
    getErrorMessage({ message: "upload rejected" }, "Unknown error"),
    "upload rejected",
  );
  assert.equal(getErrorMessage({}, "Unknown error"), "Unknown error");
});

test("address-locked agents join explicit mentions without duplicating recipients", () => {
  const explicit = ["A".repeat(64), "b".repeat(64)];
  const locked = ["a".repeat(64), "C".repeat(64)];

  assert.deepEqual(mergeMentionRecipients(explicit, locked), [
    "a".repeat(64),
    "b".repeat(64),
    "c".repeat(64),
  ]);
});

test("a remote representative prevents a stopped mentioned agent from starting here", () => {
  const agent = {
    pubkey: "fizz",
    status: "stopped",
    backend: { type: "local" },
  };
  const devices = [
    device({ current: true, standbyAgents: ["fizz"] }),
    device({ deviceId: "office", activeAgents: ["FIZZ"] }),
  ];

  assert.equal(shouldStartMentionedAgentHere(agent, devices), false);
});

test("a stopped mentioned agent starts here when no representative is online", () => {
  const agent = {
    pubkey: "fizz",
    status: "stopped",
    backend: { type: "local" },
  };
  const devices = [
    device({ current: true, standbyAgents: ["fizz"] }),
    device({ deviceId: "office", online: false, activeAgents: ["fizz"] }),
  ];

  assert.equal(shouldStartMentionedAgentHere(agent, devices), true);
});

test("provider-backed mentioned agents also reuse an online representative", () => {
  const devices = [device({ activeAgents: ["fizz"] })];
  const providerAgent = (status) => ({
    pubkey: "fizz",
    status,
    backend: { type: "provider", id: "remote", config: {} },
  });

  assert.equal(
    shouldStartMentionedAgentHere(providerAgent("stopped"), devices),
    false,
  );
  assert.equal(
    shouldStartMentionedAgentHere(providerAgent("running"), devices),
    false,
  );
  assert.equal(
    shouldStartMentionedAgentHere(providerAgent("deployed"), devices),
    false,
  );
  assert.equal(
    shouldStartMentionedAgentHere(providerAgent("stopped"), []),
    true,
  );
  assert.equal(
    shouldStartMentionedAgentHere(providerAgent("deployed"), []),
    false,
  );
});

test("revalidation carries captured and prepared agent keys independently of the cleared composer", () => {
  const draft = {
    inlineAgentMentionPubkeys: ["A".repeat(64)],
    addressedAgentPubkeys: ["b".repeat(64)],
  };
  assert.deepEqual(mentionRevalidationOptions(draft, "prepare"), {
    phase: "prepare",
    intendedAgentPubkeys: ["a".repeat(64), "b".repeat(64)],
  });
  assert.deepEqual(
    mentionRevalidationOptions(draft, "publish", [
      "a".repeat(64),
      "c".repeat(64),
    ]),
    {
      phase: "publish",
      intendedAgentPubkeys: ["a".repeat(64), "b".repeat(64), "c".repeat(64)],
    },
  );
});
