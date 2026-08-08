import type { ManagedAgentBackend } from "@/shared/api/types";

export type RunOnConfigRow = {
  key: string;
  label: string;
  value: string;
  redacted: boolean;
};

export type RunOnSummary =
  | { location: "local" }
  | { location: "provider"; providerId: string; rows: RunOnConfigRow[] };

const SECRET_WORDS = new Set([
  "secret",
  "password",
  "token",
  "key",
  "credential",
  "passphrase",
  "auth",
  "nsec",
]);

const REDACTED_PLACEHOLDER = "••••••••";
const LABEL_ACRONYMS = new Set(["cpu", "id", "url", "api"]);

function splitConfigKey(key: string): string[] {
  return key
    .replace(/([a-z0-9])([A-Z])/g, "$1 $2")
    .replace(/([A-Z]+)([A-Z][a-z])/g, "$1 $2")
    .split(/[_\s.-]+/)
    .filter((word) => word.length > 0)
    .map((word) => word.toLowerCase());
}

export function humanizeConfigKey(key: string): string {
  const words = splitConfigKey(key);
  if (words.length === 0) return key;
  return words
    .map((word, index) => {
      if (LABEL_ACRONYMS.has(word)) return word.toUpperCase();
      if (index === 0) return word.charAt(0).toUpperCase() + word.slice(1);
      return word;
    })
    .join(" ");
}

function displayValue(value: unknown): string {
  if (value === null || value === undefined) return "Not set";
  if (typeof value === "string") return value.length > 0 ? value : "Not set";
  if (typeof value === "number" || typeof value === "boolean")
    return String(value);
  return Array.isArray(value)
    ? `List (${value.length} items)`
    : "Structured value";
}

const PREFERRED_KEY_ORDER = [
  "context",
  "namespace",
  "image",
  "cpu_request",
  "memory_request",
  "cpu_limit",
  "memory_limit",
  "inactivity_seconds",
  "service_account",
];

function compareKeys(a: string, b: string): number {
  const ia = PREFERRED_KEY_ORDER.indexOf(a);
  const ib = PREFERRED_KEY_ORDER.indexOf(b);
  if (ia !== -1 && ib !== -1) return ia - ib;
  if (ia !== -1) return -1;
  if (ib !== -1) return 1;
  return a.localeCompare(b);
}

export function summarizeRunOn(backend: ManagedAgentBackend): RunOnSummary {
  if (backend.type === "local") return { location: "local" };
  const rows = Object.entries(backend.config ?? {})
    .sort(([a], [b]) => compareKeys(a, b))
    .map(([key, value]): RunOnConfigRow => {
      const redacted = splitConfigKey(key).some((word) =>
        SECRET_WORDS.has(word),
      );
      return {
        key,
        label: humanizeConfigKey(key),
        value: redacted ? REDACTED_PLACEHOLDER : displayValue(value),
        redacted,
      };
    });
  return { location: "provider", providerId: backend.id, rows };
}
