import assert from "node:assert/strict";
import test from "node:test";

import {
  MAJU_ACP_CONFIG_OPTIONS,
  acpConfigScalarFromToken,
  acpConfigScalarToken,
  configOptionByCategory,
  readStoredAcpConfigOptions,
  renderableAdvancedAcpOptions,
  writeStoredAcpConfigOption,
} from "./acpNativeOptions.ts";

test("typed ACP values round-trip without string coercion", () => {
  for (const value of ["high", true, false, 7]) {
    assert.deepEqual(
      acpConfigScalarFromToken(acpConfigScalarToken(value)),
      value,
    );
  }
});

test("stored ACP options preserve unrelated values and retain an explicit empty envelope", () => {
  const initial = {
    KEEP: "yes",
    [MAJU_ACP_CONFIG_OPTIONS]: JSON.stringify({ "fast-mode": false }),
  };
  const withEffort = writeStoredAcpConfigOption(
    initial,
    "reasoningEffort",
    "high",
  );
  assert.deepEqual(readStoredAcpConfigOptions(withEffort), {
    "fast-mode": false,
    reasoningEffort: "high",
  });

  const withoutFast = writeStoredAcpConfigOption(withEffort, "fast-mode", null);
  const empty = writeStoredAcpConfigOption(
    withoutFast,
    "reasoningEffort",
    null,
  );
  assert.equal(empty.KEEP, "yes");
  assert.equal(empty[MAJU_ACP_CONFIG_OPTIONS], "{}");
  assert.deepEqual(readStoredAcpConfigOptions(empty), {});
});

test("categories, not adapter names, decide native field placement", () => {
  const options = [
    {
      configId: "model-choice",
      category: "model",
      displayName: "Model",
      description: null,
      optionType: "select",
      currentValue: "base",
      options: [],
    },
    {
      configId: "reasoningEffort",
      category: "thought_level",
      displayName: "Reasoning",
      description: null,
      optionType: "select",
      currentValue: "medium",
      options: [{ value: "medium", displayName: "Medium" }],
    },
    {
      configId: "fast-mode",
      category: null,
      displayName: "Fast mode",
      description: null,
      optionType: "boolean",
      currentValue: false,
      options: [],
    },
  ];

  assert.equal(
    configOptionByCategory(options, "thought_level")?.configId,
    "reasoningEffort",
  );
  assert.deepEqual(
    renderableAdvancedAcpOptions(options).map((option) => option.configId),
    ["fast-mode"],
  );
});
