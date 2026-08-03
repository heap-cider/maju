import assert from "node:assert/strict";
import test from "node:test";

import {
  findAgentExecutionLocation,
  indexAgentExecutionLocations,
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
