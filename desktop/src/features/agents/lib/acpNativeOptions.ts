import type { AcpConfigOptionEntry, AcpConfigScalar } from "@/shared/api/types";

export const MAJU_ACP_CONFIG_OPTIONS = "MAJU_ACP_CONFIG_OPTIONS";

export type StoredAcpConfigOptions = Record<string, AcpConfigScalar>;

export function readStoredAcpConfigOptions(
  envVars: Record<string, string>,
): StoredAcpConfigOptions {
  const raw = envVars[MAJU_ACP_CONFIG_OPTIONS];
  if (!raw) return {};
  try {
    const parsed: unknown = JSON.parse(raw);
    if (
      typeof parsed !== "object" ||
      parsed === null ||
      Array.isArray(parsed)
    ) {
      return {};
    }
    return Object.fromEntries(
      Object.entries(parsed).filter(
        (entry): entry is [string, AcpConfigScalar] =>
          typeof entry[1] === "string" ||
          typeof entry[1] === "number" ||
          typeof entry[1] === "boolean",
      ),
    );
  } catch {
    return {};
  }
}

export function writeStoredAcpConfigOption(
  envVars: Record<string, string>,
  configId: string,
  value: AcpConfigScalar | null,
): Record<string, string> {
  const options = readStoredAcpConfigOptions(envVars);
  if (value === null) delete options[configId];
  else options[configId] = value;

  const next = { ...envVars };
  // Preserve an explicit empty map. Definition sync treats `{}` as a clear,
  // while an absent key means a legacy event did not carry this field.
  next[MAJU_ACP_CONFIG_OPTIONS] = JSON.stringify(options);
  return next;
}

export function configOptionByCategory(
  options: readonly AcpConfigOptionEntry[],
  category: string,
): AcpConfigOptionEntry | null {
  return options.find((option) => option.category === category) ?? null;
}

export function renderableAdvancedAcpOptions(
  options: readonly AcpConfigOptionEntry[],
): AcpConfigOptionEntry[] {
  return options.filter(
    (option) =>
      option.category !== "model" &&
      option.category !== "mode" &&
      option.category !== "thought_level" &&
      (option.optionType === "boolean" || option.options.length > 0),
  );
}

export function acpConfigScalarToken(value: AcpConfigScalar): string {
  return JSON.stringify(value);
}

export function acpConfigScalarFromToken(
  token: string,
): AcpConfigScalar | null {
  if (token === "") return null;
  try {
    const value: unknown = JSON.parse(token);
    return typeof value === "string" ||
      typeof value === "number" ||
      typeof value === "boolean"
      ? value
      : null;
  } catch {
    return null;
  }
}

export function acpConfigScalarLabel(value: AcpConfigScalar | null): string {
  if (value === null) return "engine default";
  if (typeof value === "boolean") return value ? "On" : "Off";
  return String(value);
}
