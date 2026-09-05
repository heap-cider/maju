import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { mkdirSync, mkdtempSync, rmSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const desktopRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);
const tauriPackageJsonPath = fileURLToPath(
  import.meta.resolve("@tauri-apps/cli/package.json"),
);
const tauriPackage = JSON.parse(readFileSync(tauriPackageJsonPath, "utf8"));
const defaultTauriEntrypoint = path.resolve(
  path.dirname(tauriPackageJsonPath),
  tauriPackage.bin.tauri,
);

function runTauri(args, options = {}) {
  const entrypoint =
    process.env.MAJU_TAURI_CLI_ENTRYPOINT ?? defaultTauriEntrypoint;
  const result = spawnSync(process.execPath, [entrypoint, ...args], {
    cwd: desktopRoot,
    env: { ...process.env, ...options.env },
    stdio: "inherit",
  });
  if (result.error) throw result.error;
  return result.status ?? 1;
}

export function runTauriCommand(args) {
  if (args[0] !== "build") return runTauri(args);

  // Tauri runs beforeBuildCommand and then consumes frontendDist. Give the
  // entire invocation a private directory so concurrent OSS/internal packages
  // cannot replace one another's assets between those two operations.
  const tauriRoot = path.join(desktopRoot, "src-tauri");
  const assetRoot = path.join(tauriRoot, "target");
  mkdirSync(assetRoot, { recursive: true });
  const invocationRoot = mkdtempSync(
    path.join(assetRoot, "maju-tauri-package-assets-"),
  );
  const frontendDist = path.join(invocationRoot, "dist");
  // Tauri parses drive-letter paths as URLs and then embeds no assets.
  // Keep the directory on the same drive so this is always a relative path.
  const outputOverride = JSON.stringify({
    build: { frontendDist: path.relative(tauriRoot, frontendDist) },
  });

  try {
    const delimiterIndex = args.indexOf("--");
    const configIndex = delimiterIndex === -1 ? args.length : delimiterIndex;
    const tauriArgs = [...args];
    tauriArgs.splice(configIndex, 0, "--config", outputOverride);
    return runTauri(tauriArgs, {
      env: { MAJU_PROTECTED_BUILD_OUTPUT: frontendDist },
    });
  } finally {
    rmSync(invocationRoot, { recursive: true, force: true });
  }
}

if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  process.exitCode = runTauriCommand(process.argv.slice(2));
}
