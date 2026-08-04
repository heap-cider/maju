import assert from "node:assert/strict";
import test from "node:test";

import { deriveAgentConfigFieldModel } from "./agentConfigCore.ts";

const config = {
  env_vars: { MAJU_AGENT_THINKING_EFFORT: "high" },
  model: "test-model",
  preferred_runtime: null,
  provider: "anthropic",
};

function runtime(id, metadata = {}) {
  return {
    id,
    label: id,
    avatarUrl: "",
    availability: "available",
    command: id,
    binaryPath: id,
    defaultArgs: [],
    mcpCommand: null,
    modelEnvVar: null,
    providerEnvVar: null,
    thinkingEnvVar: null,
    installHint: "",
    installInstructionsUrl: "",
    canAutoInstall: false,
    underlyingCliPath: null,
    nodeRequired: false,
    authStatus: { status: "not_applicable" },
    loginHint: null,
    ...metadata,
  };
}

function field(model, kind) {
  return model.fields.find((candidate) => candidate.kind === kind);
}

test("Maju Agent exposes provider, model, and Maju-owned effort", () => {
  const model = deriveAgentConfigFieldModel({
    config,
    runtime: runtime("maju-agent", {
      modelEnvVar: "MAJU_AGENT_MODEL",
      providerEnvVar: "MAJU_AGENT_PROVIDER",
      thinkingEnvVar: "MAJU_AGENT_THINKING_EFFORT",
    }),
    scope: "global",
  });

  assert.deepEqual(
    model.fields.map((item) => item.kind),
    ["provider", "model", "effort"],
  );
  assert.equal(field(model, "effort").optionSource, "majuAgentCatalog");
  assert.deepEqual(field(model, "effort").targetApplication, {
    kind: "envVar",
    key: "MAJU_AGENT_THINKING_EFFORT",
  });
});

test("Goose exposes provider, model, and its real effort application key", () => {
  const model = deriveAgentConfigFieldModel({
    config,
    runtime: runtime("goose", {
      modelEnvVar: "GOOSE_MODEL",
      providerEnvVar: "GOOSE_PROVIDER",
      thinkingEnvVar: "GOOSE_THINKING_EFFORT",
    }),
    scope: "global",
  });

  assert.equal(
    field(model, "effort").optionSource,
    "legacyProviderModelCatalog",
  );
  assert.deepEqual(field(model, "effort").currentPersistence, {
    kind: "envVar",
    key: "MAJU_AGENT_THINKING_EFFORT",
  });
  assert.deepEqual(field(model, "effort").targetApplication, {
    kind: "envVar",
    key: "GOOSE_THINKING_EFFORT",
  });
});

test("ACP-native effort is deferred to category discovery without runtime IDs", () => {
  const model = deriveAgentConfigFieldModel({
    config,
    runtime: runtime("claude"),
    scope: "global",
  });

  assert.deepEqual(
    model.fields.map((item) => item.kind),
    ["model", "effort"],
  );
  assert.equal(
    field(model, "effort").render,
    "deferredUntilNativeOptionsAvailable",
  );
  assert.deepEqual(field(model, "effort").currentPersistence, {
    kind: "unavailable",
  });
  assert.deepEqual(field(model, "effort").targetApplication, {
    kind: "acpConfigOption",
    id: null,
    category: "thought_level",
  });
});

test("Codex also defers generically and renders nothing when discovery has no thought level", () => {
  const model = deriveAgentConfigFieldModel({
    config,
    runtime: runtime("codex"),
    scope: "global",
  });

  assert.deepEqual(
    model.fields.map((item) => item.kind),
    ["model", "effort"],
  );
  assert.equal(
    field(model, "effort").render,
    "deferredUntilNativeOptionsAvailable",
  );
  assert.deepEqual(model.omissions, []);
});

test("catalog mismatch cleanup is named and restricted to onboarding", () => {
  const selectedRuntime = runtime("maju-agent", {
    modelEnvVar: "MAJU_AGENT_MODEL",
    providerEnvVar: "MAJU_AGENT_PROVIDER",
    thinkingEnvVar: "MAJU_AGENT_THINKING_EFFORT",
  });
  const onboarding = deriveAgentConfigFieldModel({
    config,
    runtime: selectedRuntime,
    scope: "onboarding",
  });
  const evergreen = deriveAgentConfigFieldModel({
    config,
    runtime: selectedRuntime,
    scope: "instance",
  });

  assert.deepEqual(onboarding.dependentValuePolicy, {
    onContextChange: "resetDependentValues",
    onCatalogMismatch: "onboardingCleanup",
  });
  assert.deepEqual(evergreen.dependentValuePolicy, {
    onContextChange: "resetDependentValues",
    onCatalogMismatch: "explainOnly",
  });
});
