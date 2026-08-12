import assert from "node:assert/strict";
import test from "node:test";

import {
  findAgentExecutionLocation,
  hasOnlineAgentRepresentative,
  indexAgentExecutionLocations,
  removeAgentFromCurrentDeviceSnapshot,
} from "./agentExecutionLocations.ts";

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

test("shows the remote representative and this device standby together", () => {
  const locations = indexAgentExecutionLocations([
    device({
      deviceId: "home",
      name: "집 PC",
      current: true,
      standbyAgents: ["FIZZ"],
    }),
    device({
      deviceId: "office",
      name: "사무실 PC",
      activeAgents: ["fizz"],
    }),
  ]);

  assert.deepEqual(findAgentExecutionLocation(locations, " fizz "), {
    representativeDeviceName: "사무실 PC",
    representativeIsCurrent: false,
    currentDeviceIsStandby: true,
  });
});

test("marks the current device when it owns the representative lease", () => {
  const locations = indexAgentExecutionLocations([
    device({
      name: "집 PC",
      current: true,
      activeAgents: ["fizz"],
    }),
  ]);

  assert.deepEqual(findAgentExecutionLocation(locations, "FIZZ"), {
    representativeDeviceName: "집 PC",
    representativeIsCurrent: true,
    currentDeviceIsStandby: false,
  });
});

test("keeps a standby snapshot useful during a short representative race", () => {
  const locations = indexAgentExecutionLocations([
    device({ current: true, standbyAgents: ["fizz"] }),
  ]);

  assert.deepEqual(findAgentExecutionLocation(locations, "fizz"), {
    representativeDeviceName: null,
    representativeIsCurrent: false,
    currentDeviceIsStandby: true,
  });
});

test("representative check ignores standby and offline devices", () => {
  const devices = [
    device({ online: false, activeAgents: ["fizz"] }),
    device({ current: true, standbyAgents: ["fizz"] }),
  ];

  assert.equal(hasOnlineAgentRepresentative(devices, "FIZZ"), false);
});

test("representative check accepts an online runner on another device", () => {
  const devices = [
    device({ current: true, standbyAgents: ["fizz"] }),
    device({ deviceId: "office", activeAgents: [" FIZZ "] }),
  ];

  assert.equal(hasOnlineAgentRepresentative(devices, "fizz"), true);
});

test("stopping here clears only this device from the cached runner snapshot", () => {
  const devices = [
    device({
      current: true,
      activeAgents: ["fizz", "keep"],
      standbyAgents: ["FIZZ"],
    }),
    device({ deviceId: "office", activeAgents: ["fizz"] }),
  ];

  const updated = removeAgentFromCurrentDeviceSnapshot(devices, " FIZZ ");

  assert.deepEqual(updated[0].activeAgents, ["keep"]);
  assert.deepEqual(updated[0].standbyAgents, []);
  assert.deepEqual(updated[1].activeAgents, ["fizz"]);
});
