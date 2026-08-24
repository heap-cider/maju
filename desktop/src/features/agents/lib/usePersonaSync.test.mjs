import assert from "node:assert/strict";
import test, { mock } from "node:test";

import { relayClient } from "@/shared/api/relayClient";
import {
  KIND_DELETION,
  KIND_MANAGED_AGENT,
  KIND_PERSONA,
  KIND_TEAM,
} from "@/shared/constants/kinds";
import {
  backfillPersonaSync,
  coalesceManagedAgentBackfill,
  startPersonaSync,
} from "./usePersonaSync.ts";

const EXPECTED_KINDS = [
  KIND_PERSONA,
  KIND_TEAM,
  KIND_MANAGED_AGENT,
  KIND_DELETION,
];

function event({
  id,
  kind = KIND_MANAGED_AGENT,
  createdAt,
  pubkey = "owner-pubkey",
  dTag = "agent-pubkey",
}) {
  return {
    id,
    pubkey,
    created_at: createdAt,
    kind,
    tags: dTag ? [["d", dTag]] : [],
    content: "{}",
    sig: "sig",
  };
}

test("startup backfill keeps only the newest managed-agent head per coordinate", () => {
  const persona = event({
    id: "persona",
    kind: KIND_PERSONA,
    createdAt: 1,
    dTag: "persona-id",
  });
  const otherAgent = event({
    id: "other-agent",
    createdAt: 2,
    dTag: "other-agent",
  });
  const oldest = event({ id: "oldest", createdAt: 1 });
  const sameSecondLoser = event({ id: "f", createdAt: 3 });
  const newest = event({ id: "a", createdAt: 3 });

  assert.deepEqual(
    coalesceManagedAgentBackfill([
      oldest,
      persona,
      newest,
      otherAgent,
      sameSecondLoser,
    ]).map(({ id }) => id),
    ["persona", "a", "other-agent"],
    "NIP-33 uses newest created_at and lowest id on a tie",
  );
});

// Regression guard for the fresh-start backfill gap (F3): a device that comes
// online AFTER another published gets zero history from a live-only `limit: 0`
// subscription, because reconnect-replay's since-cursor is undefined until the
// first live event. `startPersonaSync` MUST do a one-shot history fetch up
// front, and both the backfill and the live sub MUST carry the deletion kind
// so tombstones catch up too.
test("startPersonaSync backfills history including the deletion kind", async () => {
  const fetchCalls = [];
  const liveCalls = [];
  mock.method(relayClient, "fetchEventsWithOrigin", (filter) => {
    fetchCalls.push(filter);
    return Promise.resolve({ events: [], relayUrl: "wss://relay.example" });
  });
  mock.method(relayClient, "subscribeLiveWithOrigin", (filter) => {
    liveCalls.push(filter);
    return Promise.resolve(() => Promise.resolve());
  });

  startPersonaSync("owner-pubkey", "wss://relay.example", () => false);

  assert.equal(fetchCalls.length, 1, "must do exactly one backfill fetch");
  assert.deepEqual(
    fetchCalls[0].kinds,
    EXPECTED_KINDS,
    "backfill must cover persona/team/agent + deletion",
  );
  assert.ok(
    fetchCalls[0].limit > 0,
    "backfill must request a positive limit — limit:0 returns no history",
  );
  assert.deepEqual(fetchCalls[0].authors, ["owner-pubkey"]);

  assert.equal(liveCalls.length, 1);
  assert.deepEqual(
    liveCalls[0].kinds,
    EXPECTED_KINDS,
    "live sub must also carry the deletion kind",
  );

  await new Promise((resolve) => setImmediate(resolve));

  mock.reset();
});

// Regression guard for the arrival-scope fix (F6): the reconcile must carry the
// relay this subscription was opened on, NOT whichever community happens to be
// active when the reconcile runs. Without the forwarded URL the backend falls
// back to the active workspace and an in-flight event lands in the wrong
// community's scoped retention store on a mid-flight switch.
test("startPersonaSync forwards its own relay as the event arrival relay", async () => {
  const invokes = [];
  // @tauri-apps/api/core reads `window.__TAURI_INTERNALS__.invoke`.
  globalThis.window = {
    __TAURI_INTERNALS__: {
      invoke: (cmd, args) => {
        invokes.push({ cmd, args });
        return Promise.resolve();
      },
    },
  };

  const ownEvent = { id: "e1", pubkey: "owner-pubkey", kind: KIND_PERSONA };
  const foreignEvent = { id: "e2", pubkey: "someone-else", kind: KIND_PERSONA };

  mock.method(relayClient, "fetchEventsWithOrigin", () =>
    Promise.resolve({
      events: [ownEvent, foreignEvent],
      relayUrl: "wss://community-a.example",
    }),
  );
  mock.method(relayClient, "subscribeLiveWithOrigin", () =>
    Promise.resolve(() => Promise.resolve()),
  );

  startPersonaSync("owner-pubkey", "wss://community-a.example", () => false);
  // Let the backfill promise chain and the reconcile invoke settle.
  await new Promise((resolve) => setImmediate(resolve));

  const reconciles = invokes.filter(
    (call) => call.cmd === "reconcile_inbound_persona_event",
  );
  assert.equal(
    reconciles.length,
    1,
    "only the subscribed author's event reconciles",
  );
  assert.equal(
    reconciles[0].args.arrivalRelayUrl,
    "wss://community-a.example",
    "reconcile must carry the subscription's relay as the arrival relay",
  );
  assert.equal(JSON.parse(reconciles[0].args.eventJson).id, "e1");

  mock.reset();
  delete globalThis.window;
});

test("startPersonaSync drops backfill and live events whose socket origin differs", async () => {
  const invokes = [];
  globalThis.window = {
    __TAURI_INTERNALS__: {
      invoke: (cmd, args) => {
        invokes.push({ cmd, args });
        return Promise.resolve();
      },
    },
  };

  mock.method(relayClient, "fetchEventsWithOrigin", () =>
    Promise.resolve({
      events: [
        { id: "office-event", pubkey: "owner-pubkey", kind: KIND_PERSONA },
      ],
      relayUrl: "wss://office.example",
    }),
  );
  let onLiveEvent;
  mock.method(relayClient, "subscribeLiveWithOrigin", (_filter, listener) => {
    onLiveEvent = listener;
    return Promise.resolve(() => Promise.resolve());
  });

  startPersonaSync("owner-pubkey", "wss://maju.example", () => false);
  await new Promise((resolve) => setImmediate(resolve));
  onLiveEvent(
    {
      id: "office-live",
      pubkey: "owner-pubkey",
      kind: KIND_PERSONA,
    },
    "wss://office.example",
  );
  await new Promise((resolve) => setImmediate(resolve));

  assert.equal(
    invokes.filter((call) => call.cmd === "reconcile_inbound_persona_event")
      .length,
    0,
    "an intended label must never relabel events from another socket",
  );

  mock.reset();
  delete globalThis.window;
});
test("provisioning can await every backfilled identity write", async () => {
  const invokes = [];
  let releaseFirstWrite;
  const firstWrite = new Promise((resolve) => {
    releaseFirstWrite = resolve;
  });
  globalThis.window = {
    __TAURI_INTERNALS__: {
      invoke: async (cmd, args) => {
        invokes.push({ cmd, args });
        if (invokes.length === 1) await firstWrite;
      },
    },
  };

  mock.method(relayClient, "fetchEventsWithOrigin", () =>
    Promise.resolve({
      events: [
        { id: "old", pubkey: "owner-pubkey", kind: KIND_MANAGED_AGENT },
        { id: "new", pubkey: "owner-pubkey", kind: KIND_MANAGED_AGENT },
      ],
      relayUrl: "wss://community-a.example",
    }),
  );

  let settled = false;
  const backfill = backfillPersonaSync(
    "owner-pubkey",
    "wss://community-a.example",
  ).then(() => {
    settled = true;
  });
  await new Promise((resolve) => setImmediate(resolve));

  assert.equal(invokes.length, 1, "writes must be applied in relay order");
  assert.equal(settled, false, "backfill must wait for the local store write");

  releaseFirstWrite();
  await backfill;
  assert.equal(invokes.length, 2);
  assert.equal(settled, true);

  mock.reset();
  delete globalThis.window;
});

test("startPersonaSync serializes inbound reconciliation in relay order", async () => {
  const resolvers = [];
  const invokedIds = [];
  globalThis.window = {
    __TAURI_INTERNALS__: {
      invoke: (_cmd, args) => {
        invokedIds.push(JSON.parse(args.eventJson).id);
        return new Promise((resolve) => resolvers.push(resolve));
      },
    },
  };

  let onEvent;
  mock.method(relayClient, "fetchEventsWithOrigin", () =>
    Promise.resolve({ events: [], relayUrl: "wss://community.example" }),
  );
  mock.method(relayClient, "subscribeLiveWithOrigin", (_filter, listener) => {
    onEvent = listener;
    return Promise.resolve(() => Promise.resolve());
  });

  startPersonaSync("owner-pubkey", "wss://community.example", () => false);
  await new Promise((resolve) => setImmediate(resolve));
  onEvent(
    { id: "broad", pubkey: "owner-pubkey", kind: KIND_MANAGED_AGENT },
    "wss://community.example",
  );
  onEvent(
    {
      id: "restricted",
      pubkey: "owner-pubkey",
      kind: KIND_MANAGED_AGENT,
    },
    "wss://community.example",
  );
  await new Promise((resolve) => setImmediate(resolve));

  assert.deepEqual(
    invokedIds,
    ["broad"],
    "newer event waits for prior deployment",
  );
  resolvers.shift()();
  await new Promise((resolve) => setImmediate(resolve));
  assert.deepEqual(invokedIds, ["broad", "restricted"]);
  resolvers.shift()();

  mock.reset();
  delete globalThis.window;
});
