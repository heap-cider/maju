import assert from "node:assert/strict";
import test from "node:test";

import { humanizeConfigKey, summarizeRunOn } from "./runOnSummary.ts";

test("local backend summarizes to the local location with no rows", () => {
  assert.deepEqual(summarizeRunOn({ type: "local" }), { location: "local" });
});

test("provider backend carries the provider id", () => {
  const summary = summarizeRunOn({
    type: "provider",
    id: "kubernetes",
    config: {},
  });
  assert.equal(summary.location, "provider");
  assert.equal(summary.providerId, "kubernetes");
  assert.deepEqual(summary.rows, []);
});

test("a saved provider config renders labeled scalar rows", () => {
  const summary = summarizeRunOn({
    type: "provider",
    id: "kubernetes",
    config: {
      namespace: "maju-agents-x7k2mp",
      image: "ghcr.io/heap-cider/maju-sprig@sha256:17facfc7",
      cpu_request: "1",
      inactivity_seconds: 7200,
    },
  });
  assert.equal(summary.location, "provider");
  const byKey = Object.fromEntries(summary.rows.map((row) => [row.key, row]));
  assert.equal(byKey.namespace.label, "Namespace");
  assert.equal(byKey.namespace.value, "maju-agents-x7k2mp");
  assert.equal(byKey.cpu_request.label, "CPU request");
  assert.equal(byKey.cpu_request.value, "1");
  assert.equal(byKey.inactivity_seconds.label, "Inactivity seconds");
  assert.equal(byKey.inactivity_seconds.value, "7200");
  assert.ok(summary.rows.every((row) => row.redacted === false));
});

test("rows follow preferred order with alphabetical spillover", () => {
  const summary = summarizeRunOn({
    type: "provider",
    id: "kubernetes",
    config: {
      memory_limit: "1Gi",
      zeta_extra: "z",
      namespace: "n",
      cpu_limit: "1",
      alpha_extra: "a",
      image: "i",
      inactivity_seconds: 7200,
      cpu_request: "1",
      context: "c",
      memory_request: "1Gi",
    },
  });
  assert.deepEqual(
    summary.rows.map((row) => row.key),
    [
      "context",
      "namespace",
      "image",
      "cpu_request",
      "memory_request",
      "cpu_limit",
      "memory_limit",
      "inactivity_seconds",
      "alpha_extra",
      "zeta_extra",
    ],
  );
});

test("scalar and structured values display safely", () => {
  const summary = summarizeRunOn({
    type: "provider",
    id: "provider",
    config: {
      a_null: null,
      b_empty: "",
      c_flag: true,
      d_num: 0,
      resources: { cpu: "2" },
      tolerations: ["spot"],
    },
  });
  const values = Object.fromEntries(
    summary.rows.map((row) => [row.key, row.value]),
  );
  assert.equal(values.a_null, "Not set");
  assert.equal(values.b_empty, "Not set");
  assert.equal(values.c_flag, "true");
  assert.equal(values.d_num, "0");
  assert.equal(values.resources, "Structured value");
  assert.equal(values.tolerations, "List (1 items)");
});

test("secret-shaped keys are redacted", () => {
  const summary = summarizeRunOn({
    type: "provider",
    id: "future-provider",
    config: {
      api_token: "abc123",
      registry_password: "hunter2",
      clientSecret: "s3cr3t",
      authHeader: "Bearer xyz",
      privateKey: "nsec1...",
      namespace: "safe-to-show",
    },
  });
  const byKey = Object.fromEntries(summary.rows.map((row) => [row.key, row]));
  for (const key of [
    "api_token",
    "registry_password",
    "clientSecret",
    "authHeader",
    "privateKey",
  ]) {
    assert.equal(byKey[key].redacted, true);
    assert.equal(byKey[key].value, "••••••••");
  }
  assert.equal(byKey.namespace.redacted, false);
});

test("humanizeConfigKey handles common key styles", () => {
  assert.equal(humanizeConfigKey("cpu_request"), "CPU request");
  assert.equal(humanizeConfigKey("serviceAccount"), "Service account");
  assert.equal(humanizeConfigKey("api_url"), "API URL");
  assert.equal(humanizeConfigKey(""), "");
});
