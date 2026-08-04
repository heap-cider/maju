import * as React from "react";

import type { AcpConfigOptionEntry, AcpConfigScalar } from "@/shared/api/types";
import { cn } from "@/shared/lib/cn";
import {
  acpConfigScalarFromToken,
  acpConfigScalarLabel,
  acpConfigScalarToken,
  configOptionByCategory,
  readStoredAcpConfigOptions,
  renderableAdvancedAcpOptions,
  writeStoredAcpConfigOption,
} from "@/features/agents/lib/acpNativeOptions";
import {
  AgentDropdownSelect,
  type AgentDropdownOption,
} from "./agentConfigControls";

function optionChoices(
  option: AcpConfigOptionEntry,
  storedValue: AcpConfigScalar | undefined,
  inheritedValue: AcpConfigScalar | undefined,
): AgentDropdownOption[] {
  const defaultLabel =
    inheritedValue !== undefined
      ? `Inherit (${acpConfigScalarLabel(inheritedValue)})`
      : option.currentValue === null
        ? "Engine default"
        : `Engine default (${acpConfigScalarLabel(option.currentValue)})`;
  const choices: AgentDropdownOption[] = [{ label: defaultLabel, value: "" }];
  if (option.optionType === "boolean") {
    choices.push(
      { label: "On", value: acpConfigScalarToken(true) },
      { label: "Off", value: acpConfigScalarToken(false) },
    );
  } else {
    choices.push(
      ...option.options.map((choice) => ({
        label: choice.displayName ?? choice.value,
        value: acpConfigScalarToken(choice.value),
      })),
    );
  }

  if (
    storedValue !== undefined &&
    !choices.some(
      (choice) => choice.value === acpConfigScalarToken(storedValue),
    )
  ) {
    choices.push({
      disabled: true,
      label: `Unavailable (${acpConfigScalarLabel(storedValue)})`,
      value: acpConfigScalarToken(storedValue),
    });
  }
  return choices;
}

function NativeOptionField({
  disabled,
  envVars,
  fieldClassName,
  inheritedEnvVars,
  labelClassName,
  onEnvVarsChange,
  option,
  selectClassName,
  testId,
  useCustomSelect,
}: {
  disabled: boolean;
  envVars: Record<string, string>;
  fieldClassName?: string;
  inheritedEnvVars: Record<string, string>;
  labelClassName?: string;
  onEnvVarsChange: (next: Record<string, string>) => void;
  option: AcpConfigOptionEntry;
  selectClassName?: string;
  testId: string;
  useCustomSelect: boolean;
}) {
  const stored = readStoredAcpConfigOptions(envVars)[option.configId];
  const inherited =
    readStoredAcpConfigOptions(inheritedEnvVars)[option.configId];
  const value = stored === undefined ? "" : acpConfigScalarToken(stored);
  const choices = optionChoices(option, stored, inherited);
  const id = `acp-option-${option.configId.replace(/[^a-zA-Z0-9_-]/g, "-")}`;
  const handleChange = (token: string) => {
    onEnvVarsChange(
      writeStoredAcpConfigOption(
        envVars,
        option.configId,
        acpConfigScalarFromToken(token),
      ),
    );
  };

  return (
    <div className={cn("space-y-1.5", fieldClassName)}>
      <label className={cn("text-sm font-medium", labelClassName)} htmlFor={id}>
        {option.displayName ?? option.configId}
      </label>
      {useCustomSelect ? (
        <AgentDropdownSelect
          className={selectClassName}
          disabled={disabled}
          id={id}
          onValueChange={handleChange}
          options={choices}
          testId={testId}
          value={value}
        />
      ) : (
        <select
          className={cn(
            "flex h-9 w-full rounded-md border border-input bg-background px-3 py-2 text-sm shadow-xs disabled:cursor-not-allowed disabled:opacity-60",
            selectClassName,
          )}
          data-testid={testId}
          disabled={disabled}
          id={id}
          onChange={(event) => handleChange(event.target.value)}
          value={value}
        >
          {choices.map((choice) => (
            <option
              disabled={choice.disabled}
              key={choice.value}
              value={choice.value}
            >
              {choice.label}
            </option>
          ))}
        </select>
      )}
      {option.description ? (
        <p className="text-xs text-muted-foreground">{option.description}</p>
      ) : null}
      {stored !== undefined &&
      !choices.some(
        (choice) =>
          !choice.disabled && choice.value === acpConfigScalarToken(stored),
      ) ? (
        <p className="text-xs text-amber-600 dark:text-amber-400">
          This saved value is not available for the selected model. The engine
          default will be used until you choose another value.
        </p>
      ) : null}
    </div>
  );
}

export function AcpThoughtLevelField({
  configOptions,
  disabled = false,
  envVars,
  fieldClassName,
  inheritedEnvVars = {},
  labelClassName,
  onEnvVarsChange,
  selectClassName,
  useCustomSelect = false,
}: {
  configOptions: readonly AcpConfigOptionEntry[];
  disabled?: boolean;
  envVars: Record<string, string>;
  fieldClassName?: string;
  inheritedEnvVars?: Record<string, string>;
  labelClassName?: string;
  onEnvVarsChange: (next: Record<string, string>) => void;
  selectClassName?: string;
  useCustomSelect?: boolean;
}) {
  const option = configOptionByCategory(configOptions, "thought_level");
  if (
    !option ||
    (option.optionType !== "boolean" && option.options.length === 0)
  ) {
    return null;
  }
  return (
    <NativeOptionField
      disabled={disabled}
      envVars={envVars}
      fieldClassName={fieldClassName}
      inheritedEnvVars={inheritedEnvVars}
      labelClassName={labelClassName}
      onEnvVarsChange={onEnvVarsChange}
      option={option}
      selectClassName={selectClassName}
      testId="acp-thought-level-select"
      useCustomSelect={useCustomSelect}
    />
  );
}

export function AcpAdvancedOptionFields({
  configOptions,
  disabled = false,
  envVars,
  inheritedEnvVars = {},
  onEnvVarsChange,
  useCustomSelect = false,
}: {
  configOptions: readonly AcpConfigOptionEntry[];
  disabled?: boolean;
  envVars: Record<string, string>;
  inheritedEnvVars?: Record<string, string>;
  onEnvVarsChange: (next: Record<string, string>) => void;
  useCustomSelect?: boolean;
}) {
  const options = React.useMemo(
    () => renderableAdvancedAcpOptions(configOptions),
    [configOptions],
  );
  if (options.length === 0) return null;
  return (
    <div className="space-y-4">
      <p className="text-2xs font-semibold uppercase tracking-wide text-muted-foreground">
        Engine options
      </p>
      {options.map((option) => (
        <NativeOptionField
          disabled={disabled}
          envVars={envVars}
          inheritedEnvVars={inheritedEnvVars}
          key={option.configId}
          onEnvVarsChange={onEnvVarsChange}
          option={option}
          testId={`acp-option-${option.configId}`}
          useCustomSelect={useCustomSelect}
        />
      ))}
    </div>
  );
}
