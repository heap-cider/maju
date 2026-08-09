#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { dirname, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(scriptDir, "../../../..");
const buzzUpstreamUrl = "https://github.com/block/buzz.git";
const temporaryRefs = [];

function fail(message, exitCode = 1) {
  const error = new Error(message);
  error.exitCode = exitCode;
  throw error;
}

function runGit(args, { allowFailure = false, encoding = "utf8" } = {}) {
  const result = spawnSync("git", args, {
    cwd: repoRoot,
    encoding: encoding === null ? undefined : encoding,
    maxBuffer: 256 * 1024 * 1024,
    windowsHide: true,
  });

  if (result.error) {
    fail(`Unable to run git ${args[0]}: ${result.error.message}`);
  }
  if (result.status !== 0 && !allowFailure) {
    const stderr = Buffer.isBuffer(result.stderr)
      ? result.stderr.toString("utf8")
      : (result.stderr ?? "");
    fail(`git ${args[0]} failed: ${stderr.trim() || `exit ${result.status}`}`);
  }
  return result;
}

function gitText(args, options) {
  return runGit(args, options).stdout.trim();
}

function gitBuffer(args, options) {
  const output = runGit(args, { ...options, encoding: null }).stdout;
  return Buffer.isBuffer(output) ? output : Buffer.from(output ?? "");
}

function validateTagName(value) {
  const ref = `refs/tags/${value}`;
  const validation = runGit(["check-ref-format", ref], { allowFailure: true });
  if (validation.status !== 0) {
    fail(`Invalid Git tag name: ${value}`, 2);
  }
  return value;
}

function parseArgs(argv) {
  const options = { fetch: false, json: false, expectZero: false };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--from" || argument === "--to") {
      const value = argv[index + 1];
      if (!value) fail(`Missing value for ${argument}`, 2);
      options[argument.slice(2)] = validateTagName(value);
      index += 1;
    } else if (argument === "--fetch") {
      options.fetch = true;
    } else if (argument === "--json") {
      options.json = true;
    } else if (argument === "--expect-zero") {
      options.expectZero = true;
    } else if (argument === "--help" || argument === "-h") {
      console.log(
        "Usage: compare-upstream.mjs --from <tag> --to <tag> [--fetch] [--json] [--expect-zero]",
      );
      process.exit(0);
    } else {
      fail(`Unknown argument: ${argument}`, 2);
    }
  }
  if (!options.from || !options.to) {
    fail("Both --from and --to are required", 2);
  }
  return options;
}

function fetchTags(tags) {
  const fetchedCommits = new Map();
  let index = 0;
  for (const tag of new Set(tags)) {
    const temporaryRef = `refs/maju-sync/buzz-${process.pid}-${index}`;
    index += 1;
    temporaryRefs.push(temporaryRef);
    runGit([
      "fetch",
      "--no-tags",
      "--no-write-fetch-head",
      buzzUpstreamUrl,
      `+refs/tags/${tag}:${temporaryRef}`,
    ]);
    fetchedCommits.set(
      tag,
      gitText(["rev-parse", `${temporaryRef}^{commit}`]),
    );
  }
  return fetchedCommits;
}

function cleanupTemporaryRefs() {
  for (const ref of temporaryRefs.reverse()) {
    runGit(["update-ref", "-d", ref], { allowFailure: true });
  }
}

function resolveTag(tag, fetchedCommits) {
  const fetchedCommit = fetchedCommits.get(tag);
  if (fetchedCommit) return fetchedCommit;
  const ref = `refs/tags/${tag}`;
  const exists = runGit(["show-ref", "--verify", "--quiet", ref], {
    allowFailure: true,
  });
  if (exists.status !== 0) {
    fail(
      `Tag not found locally: ${tag}. Re-run with --fetch if it exists upstream.`,
      2,
    );
  }
  return gitText(["rev-parse", `refs/tags/${tag}^{commit}`]);
}

function isAncestor(ancestor, descendant) {
  return runGit(["merge-base", "--is-ancestor", ancestor, descendant], {
    allowFailure: true,
  }).status === 0;
}

function findTreeEquivalentAncestor(commit, descendant) {
  const wantedTree = gitText(["rev-parse", `${commit}^{tree}`]);
  const history = gitText([
    "log",
    "--topo-order",
    "--format=%H %T",
    descendant,
  ]);

  for (const line of history.split("\n")) {
    const [candidate, tree] = line.trim().split(/\s+/, 2);
    if (candidate && tree === wantedTree) return candidate;
  }
  return null;
}

function resolveHistory(fromTag, toTag, fromCommit, toCommit) {
  if (isAncestor(fromCommit, toCommit)) {
    return {
      relation: "direct-ancestor",
      comparisonBaseCommit: fromCommit,
      originalFromCommit: fromCommit,
    };
  }

  // Buzz release tags can point at a release-branch commit that is then
  // squash-merged back to main. The next tag consequently descends from a
  // different commit SHA even though it contains the exact prior release
  // tree. Accept that topology only when an ancestor of the new tag has a
  // byte-identical Git tree; otherwise keep failing closed.
  const equivalentAncestor = findTreeEquivalentAncestor(fromCommit, toCommit);
  if (equivalentAncestor) {
    return {
      relation: "tree-equivalent-ancestor",
      comparisonBaseCommit: equivalentAncestor,
      originalFromCommit: fromCommit,
    };
  }

  fail(
    `${toTag} is not descended from ${fromTag}, and its history contains no ` +
      "ancestor with the same Git tree as the earlier release",
  );
}

function normalizeMajuText(value) {
  return value
    .replaceAll("BUZZ", "MAJU")
    .replaceAll("Buzz", "Maju")
    .replaceAll("buzz", "maju")
    .replaceAll("\r\n", "\n");
}

function normalizeMajuPath(value) {
  return normalizeMajuText(value);
}

function normalizedContent(buffer) {
  if (buffer.includes(0)) return { binary: true, value: buffer };
  try {
    const text = new TextDecoder("utf-8", { fatal: true }).decode(buffer);
    return {
      binary: false,
      value: Buffer.from(normalizeMajuText(text), "utf8"),
    };
  } catch {
    return { binary: true, value: buffer };
  }
}

function contentAt(commit, path) {
  return gitBuffer(["show", `${commit}:${path}`]);
}

function worktreePath(path) {
  const absolute = resolve(repoRoot, ...path.split("/"));
  const relation = relative(repoRoot, absolute);
  if (relation === ".." || relation.startsWith(`..${sep}`)) {
    fail(`Refusing path outside repository: ${path}`);
  }
  return absolute;
}

function currentContent(path) {
  const absolute = worktreePath(path);
  if (!existsSync(absolute)) return null;
  return normalizedContent(readFileSync(absolute));
}

function sameContent(left, right) {
  return left !== null && right !== null && left.value.equals(right.value);
}

function parseNameStatus(buffer) {
  if (buffer.length === 0) return [];
  const fields = buffer.toString("utf8").split("\0");
  if (fields.at(-1) === "") fields.pop();
  const changes = [];
  for (let index = 0; index < fields.length; ) {
    const status = fields[index++];
    if (!status) continue;
    if (status.startsWith("R") || status.startsWith("C")) {
      changes.push({ status, oldPath: fields[index++], newPath: fields[index++] });
    } else {
      const path = fields[index++];
      changes.push({ status, oldPath: path, newPath: path });
    }
  }
  return changes;
}

function classifyChange(change, fromCommit, toCommit) {
  const kind = change.status[0];
  const majuOldPath = normalizeMajuPath(change.oldPath);
  const majuNewPath = normalizeMajuPath(change.newPath);
  const oldContent = kind === "A"
    ? null
    : normalizedContent(contentAt(fromCommit, change.oldPath));
  const newContent = kind === "D"
    ? null
    : normalizedContent(contentAt(toCommit, change.newPath));
  const currentOld = currentContent(majuOldPath);
  const currentNew = majuNewPath === majuOldPath
    ? currentOld
    : currentContent(majuNewPath);
  const binary = Boolean(oldContent?.binary || newContent?.binary);

  let classification;
  let reason;
  if (kind === "A") {
    if (currentNew === null) {
      classification = "safe-to-apply";
      reason = "normalized destination does not exist in Maju";
    } else if (sameContent(currentNew, newContent)) {
      classification = "already-applied";
      reason = "Maju already matches the normalized upstream addition";
    } else {
      classification = binary ? "manual-review" : "conflict";
      reason = "Maju already has different content at the destination";
    }
  } else if (kind === "D") {
    if (currentOld === null) {
      classification = "already-applied";
      reason = "the normalized Maju path is already absent";
    } else if (sameContent(currentOld, oldContent)) {
      classification = "safe-to-apply";
      reason = "Maju still matches the normalized file being deleted";
    } else {
      classification = binary ? "manual-review" : "conflict";
      reason = "Maju changed the file that upstream deleted";
    }
  } else if (kind === "R" || kind === "C") {
    if (sameContent(currentNew, newContent)) {
      classification = "already-applied";
      reason = "Maju already matches the normalized rename or copy";
    } else if (sameContent(currentOld, oldContent) && currentNew === null) {
      classification = "safe-to-apply";
      reason = "the normalized source is unchanged and destination is free";
    } else {
      classification = binary ? "manual-review" : "conflict";
      reason = "the normalized rename or copy overlaps Maju changes";
    }
  } else if (sameContent(currentNew, newContent)) {
    classification = "already-applied";
    reason = "Maju already matches the normalized new upstream content";
  } else if (sameContent(currentOld, oldContent)) {
    classification = "safe-to-apply";
    reason = "Maju still matches the normalized old upstream content";
  } else {
    classification = binary ? "manual-review" : "conflict";
    reason = "Maju and upstream changed the same file differently";
  }

  return {
    status: change.status,
    upstreamOldPath: change.oldPath,
    upstreamNewPath: change.newPath,
    majuOldPath,
    majuNewPath,
    binary,
    classification,
    reason,
  };
}

function countByClassification(changes) {
  const counts = {
    "safe-to-apply": 0,
    "already-applied": 0,
    conflict: 0,
    "manual-review": 0,
  };
  for (const change of changes) counts[change.classification] += 1;
  return counts;
}

function printHuman(report) {
  console.log(`Buzz upstream comparison: ${report.from.tag} -> ${report.to.tag}`);
  console.log(`History relation: ${report.history.relation}`);
  if (report.history.relation === "tree-equivalent-ancestor") {
    console.log(
      `Equivalent prior-release commit: ${report.history.comparisonBaseCommit}`,
    );
  }
  console.log(`Commits: ${report.commitCount}`);
  console.log(`Changed files: ${report.changedFileCount}`);
  console.log(`Safe to apply: ${report.classifications["safe-to-apply"]}`);
  console.log(`Already applied: ${report.classifications["already-applied"]}`);
  console.log(`Conflicts: ${report.classifications.conflict}`);
  console.log(`Manual review: ${report.classifications["manual-review"]}`);
  for (const change of report.changes) {
    console.log(
      `[${change.classification}] ${change.status} ${change.majuOldPath}` +
        (change.majuNewPath === change.majuOldPath
          ? ""
          : ` -> ${change.majuNewPath}`),
    );
  }
  console.log(`Worktree unchanged: ${report.worktreeUnchanged}`);
  console.log(
    `RESULT commits=${report.commitCount} changed_files=${report.changedFileCount} ` +
      `conflicts=${report.classifications.conflict} manual_review=${report.classifications["manual-review"]}`,
  );
}

try {
  const options = parseArgs(process.argv.slice(2));
  const worktreeBefore = gitBuffer([
    "status",
    "--porcelain=v1",
    "-z",
    "--untracked-files=all",
  ]);
  const fetchedCommits = options.fetch
    ? fetchTags([options.from, options.to])
    : new Map();

  const fromCommit = resolveTag(options.from, fetchedCommits);
  const toCommit = resolveTag(options.to, fetchedCommits);
  const history = resolveHistory(
    options.from,
    options.to,
    fromCommit,
    toCommit,
  );

  const commitCount = Number.parseInt(
    gitText([
      "rev-list",
      "--count",
      `${history.comparisonBaseCommit}..${toCommit}`,
    ]),
    10,
  );
  const rawChanges = parseNameStatus(
    gitBuffer([
      "diff",
      "--name-status",
      "-z",
      "--find-renames",
      fromCommit,
      toCommit,
      "--",
    ]),
  );
  const changes = rawChanges.map((change) =>
    classifyChange(change, fromCommit, toCommit),
  );
  const worktreeAfter = gitBuffer([
    "status",
    "--porcelain=v1",
    "-z",
    "--untracked-files=all",
  ]);
  const worktreeUnchanged = worktreeBefore.equals(worktreeAfter);
  if (!worktreeUnchanged) {
    fail("Analyzer changed the worktree status", 3);
  }

  const report = {
    schemaVersion: 2,
    source: {
      url: buzzUpstreamUrl,
      persistentRemoteRequired: false,
    },
    history,
    from: { tag: options.from, commit: fromCommit },
    to: { tag: options.to, commit: toCommit },
    fetched: options.fetch,
    commitCount,
    changedFileCount: changes.length,
    classifications: countByClassification(changes),
    worktreeDirtyBefore: worktreeBefore.length > 0,
    worktreeUnchanged,
    changes,
  };

  if (options.json) console.log(JSON.stringify(report, null, 2));
  else printHuman(report);

  if (options.expectZero && (commitCount !== 0 || changes.length !== 0)) {
    fail("Expected a zero delta, but upstream changes were detected", 4);
  }
} catch (error) {
  console.error(`ERROR: ${error.message}`);
  process.exitCode = error.exitCode ?? 1;
} finally {
  cleanupTemporaryRefs();
}
