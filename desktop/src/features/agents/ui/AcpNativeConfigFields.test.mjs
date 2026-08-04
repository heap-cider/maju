import assert from "node:assert/strict";
import test from "node:test";
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";

import {
  AcpAdvancedOptionFields,
  AcpThoughtLevelField,
} from "./AcpNativeConfigFields.tsx";

function option(overrides) {
  return {
    configId: "model",
    category: "model",
    displayName: "Model",
    description: null,
    optionType: "select",
    currentValue: "base",
    options: [],
    ...overrides,
  };
}

test("model-only ACP output does not create a duplicate effort control", () => {
  const html = renderToStaticMarkup(
    React.createElement(AcpThoughtLevelField, {
      configOptions: [option({})],
      envVars: {},
      onEnvVarsChange() {},
    }),
  );
  assert.equal(html, "");
});

test("thought_level renders every adapter-advertised choice", () => {
  const html = renderToStaticMarkup(
    React.createElement(AcpThoughtLevelField, {
      configOptions: [
        option({
          configId: "reasoningEffort",
          category: "thought_level",
          displayName: "Reasoning effort",
          currentValue: "medium",
          options: [
            { value: "low", displayName: "Low" },
            { value: "medium", displayName: "Medium" },
            { value: "high", displayName: "High" },
          ],
        }),
      ],
      envVars: {},
      onEnvVarsChange() {},
    }),
  );
  assert.match(html, /Reasoning effort/);
  for (const label of ["Low", "Medium", "High"]) {
    assert.match(html, new RegExp(`>${label}<`));
  }
});

test("generic boolean options render in advanced settings without adapter IDs", () => {
  const html = renderToStaticMarkup(
    React.createElement(AcpAdvancedOptionFields, {
      configOptions: [
        option({
          configId: "fast-mode",
          category: null,
          displayName: "Fast mode",
          optionType: "boolean",
          currentValue: false,
        }),
      ],
      envVars: {},
      onEnvVarsChange() {},
    }),
  );
  assert.match(html, /Fast mode/);
  assert.match(html, />On</);
  assert.match(html, />Off</);
});

test("an inherited native option is named without copying it into the override", () => {
  const html = renderToStaticMarkup(
    React.createElement(AcpThoughtLevelField, {
      configOptions: [
        option({
          configId: "reasoningEffort",
          category: "thought_level",
          displayName: "Reasoning effort",
          options: [{ value: "high", displayName: "High" }],
        }),
      ],
      envVars: {},
      inheritedEnvVars: {
        MAJU_ACP_CONFIG_OPTIONS: JSON.stringify({ reasoningEffort: "high" }),
      },
      onEnvVarsChange() {},
    }),
  );
  assert.match(html, /Inherit \(high\)/);
});
