import {
  buildInstanceInputForDefinition,
  resolveStartRuntimeForDefinition,
} from "@/features/agents/lib/instanceInputForDefinition";
import { backfillPersonaSync } from "@/features/agents/lib/usePersonaSync";
import {
  addChannelMembers,
  createManagedAgent,
  deleteManagedAgent,
  getChannelMembers,
  listManagedAgents,
  removeChannelMember,
  updateManagedAgent,
} from "@/shared/api/tauri";
import { discoverAcpRuntimes } from "@/shared/api/tauriAcpDiscovery";
import { getAgentAccessOwnerOnly } from "@/shared/api/tauriAgentAccess";
import { getGlobalAgentConfig } from "@/shared/api/tauriGlobalAgentConfig";
import { getIdentity } from "@/shared/api/tauriIdentity";
import { listPersonas, setPersonaActive } from "@/shared/api/tauriPersonas";
import type {
  AcpRuntime,
  AgentPersona,
  CreateManagedAgentInput,
  ManagedAgent,
  UpdateManagedAgentInput,
} from "@/shared/api/types";
import { normalizePubkey } from "@/shared/lib/pubkey";

export const WELCOME_GUIDE_AGENT_NAME = "Fizz";
export const WELCOME_GUIDE_PERSONA_ID = "builtin:fizz";
export const WELCOME_TEAM_ID = "builtin-team:welcome";
export const WELCOME_GUIDE_INTRO_MARKER = "maju-welcome-intro.v1";
const LEGACY_WELCOME_GUIDE_AGENT_NAME = "Kit";
export const LEGACY_WELCOME_GUIDE_SYSTEM_PROMPT =
  "You are Kit, Sprout's friendly welcome guide. Help new users understand the community, channels, messages, and agents. Keep introductions concise, practical, and warm.";
export const WELCOME_GUIDE_INTRO_MESSAGE =
  "Hi, I'm Fizz. Welcome to Maju.\n\nI can help you get oriented, answer questions, and make the first few steps feel less mysterious.\n\nFeel free to ask me what else you can do in Maju, or just talk through what you want to build.";

export type WelcomeTeamRole = "lead" | "teammate";

export type WelcomeTeamStarterDefinition = Readonly<{
  name: string;
  personaId: string;
  role: WelcomeTeamRole;
}>;

/** Stable identities used to provision the Rust-seeded Welcome Team. */
export const WELCOME_TEAM_STARTERS = [
  { name: "Fizz", personaId: "builtin:fizz", role: "lead" },
  { name: "Honey", personaId: "builtin:honey", role: "teammate" },
  { name: "Pollen", personaId: "builtin:bumble", role: "teammate" },
] as const satisfies readonly WelcomeTeamStarterDefinition[];

export type WelcomeTeamAgents = [ManagedAgent, ManagedAgent, ManagedAgent];

const welcomeTeamPromises = new Map<string, Promise<WelcomeTeamAgents>>();

function normalizeRelayUrl(relayUrl: string | null | undefined) {
  const normalized = relayUrl?.trim().replace(/\/+$/, "");
  return normalized || null;
}

function isAgentScopedToRelay(agent: ManagedAgent, relayUrl?: string | null) {
  const targetRelayUrl = normalizeRelayUrl(relayUrl);
  if (!targetRelayUrl) {
    return true;
  }
  const agentRelayUrl = normalizeRelayUrl(agent.relayUrl);
  // Synced logical identities deliberately restore with an empty relay URL:
  // runtime placement is device-local, so empty means "this device's active
  // community". Treating it as a different relay made Welcome provisioning
  // mint a second identity and hid the original from duplicate cleanup.
  return agentRelayUrl === null || agentRelayUrl === targetRelayUrl;
}

function isBuiltInWelcomeGuideAgent(agent: ManagedAgent) {
  return agent.personaId === WELCOME_GUIDE_PERSONA_ID;
}

function isLegacyKitWelcomeGuideAgent(agent: ManagedAgent) {
  return (
    agent.name.trim().toLowerCase() ===
      LEGACY_WELCOME_GUIDE_AGENT_NAME.toLowerCase() &&
    agent.systemPrompt?.trim() === LEGACY_WELCOME_GUIDE_SYSTEM_PROMPT
  );
}

function isWelcomeGuideAgent(agent: ManagedAgent) {
  return (
    isBuiltInWelcomeGuideAgent(agent) || isLegacyKitWelcomeGuideAgent(agent)
  );
}

function pickAgentByStatus(agents: ManagedAgent[]) {
  return (
    agents.find((agent) => agent.status === "running") ??
    agents.find((agent) => agent.status === "deployed") ??
    agents[0] ??
    null
  );
}

function hasStarterName(
  agent: ManagedAgent,
  starter: WelcomeTeamStarterDefinition,
) {
  return agent.name.trim().toLowerCase() === starter.name.toLowerCase();
}

function compareAgentIdentityAge(left: ManagedAgent, right: ManagedAgent) {
  const leftCreatedAt = Date.parse(left.createdAt);
  const rightCreatedAt = Date.parse(right.createdAt);
  const leftTime = Number.isFinite(leftCreatedAt)
    ? leftCreatedAt
    : Number.POSITIVE_INFINITY;
  const rightTime = Number.isFinite(rightCreatedAt)
    ? rightCreatedAt
    : Number.POSITIVE_INFINITY;
  if (leftTime !== rightTime) return leftTime - rightTime;
  return left.pubkey.localeCompare(right.pubkey);
}

export function resolveWelcomeTeamStarterForRelay(
  agents: ManagedAgent[],
  starter: WelcomeTeamStarterDefinition,
  relayUrl?: string | null,
): { canonical: ManagedAgent | null; duplicates: ManagedAgent[] } {
  const candidates = agents.filter(
    (agent) =>
      agent.personaId === starter.personaId &&
      isAgentScopedToRelay(agent, relayUrl) &&
      (agent.teamId === WELCOME_TEAM_ID || hasStarterName(agent, starter)),
  );

  // The multi-device bug produced two exact-name starter identities. Older
  // data can be a mix of an untagged record and a team-tagged record, while a
  // second device can also provision two records that are both team-tagged.
  // Every device must make the same choice, so keep the oldest logical
  // identity instead of preferring whichever copy runs locally. The built-in
  // Welcome team id is reserved for these auto-provisioned starters, so two
  // exact-name team members are safe to collapse. Two untagged user-created
  // instances remain distinct.
  const hasLegacyUntaggedCandidate = candidates.some(
    (agent) => agent.teamId !== WELCOME_TEAM_ID,
  );
  const hasWelcomeTeamCandidate = candidates.some(
    (agent) => agent.teamId === WELCOME_TEAM_ID,
  );
  const allCandidatesBelongToWelcomeTeam = candidates.every(
    (agent) => agent.teamId === WELCOME_TEAM_ID,
  );
  if (
    candidates.length > 1 &&
    candidates.every((agent) => hasStarterName(agent, starter)) &&
    (allCandidatesBelongToWelcomeTeam ||
      (hasLegacyUntaggedCandidate && hasWelcomeTeamCandidate))
  ) {
    const [canonical, ...duplicates] = [...candidates].sort(
      compareAgentIdentityAge,
    );
    return { canonical: canonical ?? null, duplicates };
  }

  const teamCandidates = candidates.filter(
    (agent) => agent.teamId === WELCOME_TEAM_ID,
  );
  return {
    canonical:
      pickAgentByStatus(teamCandidates) ?? pickAgentByStatus(candidates),
    duplicates: [],
  };
}

export function pickWelcomeGuideAgent(agents: ManagedAgent[]) {
  return pickAgentByStatus(agents.filter(isWelcomeGuideAgent));
}

export function pickWelcomeGuideAgentForRelay(
  agents: ManagedAgent[],
  relayUrl?: string | null,
) {
  return pickAgentByStatus(
    agents.filter(
      (agent) =>
        isWelcomeGuideAgent(agent) && isAgentScopedToRelay(agent, relayUrl),
    ),
  );
}

/** Find the preferred managed instance for one starter persona and relay. */
export function pickWelcomeTeamStarterAgentForRelay(
  agents: ManagedAgent[],
  starter: WelcomeTeamStarterDefinition,
  relayUrl?: string | null,
) {
  return resolveWelcomeTeamStarterForRelay(agents, starter, relayUrl).canonical;
}

/** Pubkeys belonging to any managed Welcome Team persona on this relay. */
export async function getWelcomeTeamAgentPubkeys(relayUrl?: string | null) {
  const personaIds = new Set<string>(
    WELCOME_TEAM_STARTERS.map(({ personaId }) => personaId),
  );
  return (await listManagedAgents())
    .filter(
      (agent) =>
        agent.teamId === WELCOME_TEAM_ID &&
        agent.personaId !== null &&
        personaIds.has(agent.personaId) &&
        isAgentScopedToRelay(agent, relayUrl),
    )
    .map((agent) => agent.pubkey);
}

/** Legacy Fizz/Kit lookup retained for existing channel reuse checks. */
export async function getWelcomeGuideAgentPubkeys(relayUrl?: string | null) {
  return (await listManagedAgents())
    .filter(
      (agent) =>
        isWelcomeGuideAgent(agent) && isAgentScopedToRelay(agent, relayUrl),
    )
    .map((agent) => agent.pubkey);
}

export async function activateWelcomeTeamPersonasSequentially(
  inactivePersonaIds: readonly string[],
  activate: (personaId: string) => Promise<unknown>,
) {
  for (const personaId of inactivePersonaIds) {
    await activate(personaId);
  }
}

async function ensureWelcomeTeamPersonasActive() {
  const personas = await listPersonas();
  const personasById = new Map(
    personas.map((persona) => [persona.id, persona]),
  );

  for (const starter of WELCOME_TEAM_STARTERS) {
    if (!personasById.has(starter.personaId)) {
      throw new Error(`${starter.name} agent not found.`);
    }
  }

  // Persona activation is a read-modify-write operation over one shared file.
  // Run these sequentially so concurrent writes cannot lose a teammate's
  // activation and leave Welcome provisioning permanently partial.
  await activateWelcomeTeamPersonasSequentially(
    WELCOME_TEAM_STARTERS.filter(
      ({ personaId }) => !personasById.get(personaId)?.isActive,
    ).map(({ personaId }) => personaId),
    (personaId) => setPersonaActive(personaId, true),
  );
}

async function ensureWelcomeTeamMembership(
  channelId: string,
  agents: WelcomeTeamAgents,
) {
  const members = await getChannelMembers(channelId).catch(() => []);
  const memberPubkeys = new Set(
    members.map((member) => normalizePubkey(member.pubkey)),
  );
  const missingAgents = agents.filter(
    (agent) => !memberPubkeys.has(normalizePubkey(agent.pubkey)),
  );
  if (missingAgents.length === 0) {
    return;
  }

  const result = await addChannelMembers({
    channelId,
    pubkeys: missingAgents.map((agent) => agent.pubkey),
    role: "bot",
  });
  const unexpectedError = result.errors.find(
    ({ error }) => !error.toLowerCase().includes("already"),
  );
  if (unexpectedError) {
    throw new Error(unexpectedError.error);
  }
}

async function removeDuplicateWelcomeTeamAgents(
  channelId: string,
  agents: ManagedAgent[],
  relayUrl?: string | null,
) {
  const duplicatePubkeys = new Set<string>();
  for (const starter of WELCOME_TEAM_STARTERS) {
    const resolution = resolveWelcomeTeamStarterForRelay(
      agents,
      starter,
      relayUrl,
    );
    for (const duplicate of resolution.duplicates) {
      duplicatePubkeys.add(normalizePubkey(duplicate.pubkey));
    }
  }
  if (duplicatePubkeys.size === 0) return agents;

  // Remove channel membership before archiving the duplicate identity. If the
  // membership query or removal fails, abort cleanup and retry on the next
  // seed rather than leaving an archived identity counted in the roster.
  const members = await getChannelMembers(channelId);
  const memberPubkeys = new Set(
    members.map((member) => normalizePubkey(member.pubkey)),
  );
  for (const duplicatePubkey of duplicatePubkeys) {
    if (memberPubkeys.has(duplicatePubkey)) {
      await removeChannelMember(channelId, duplicatePubkey);
    }
    await deleteManagedAgent(duplicatePubkey);
  }

  return agents.filter(
    (agent) => !duplicatePubkeys.has(normalizePubkey(agent.pubkey)),
  );
}

export async function buildWelcomeStarterCreateInput(
  starter: WelcomeTeamStarterDefinition,
  persona: AgentPersona,
  runtimes: readonly AcpRuntime[],
  preferredRuntimeId: string | null,
  relayUrl?: string | null,
): Promise<CreateManagedAgentInput> {
  const { runtime } = resolveStartRuntimeForDefinition(
    persona,
    runtimes,
    preferredRuntimeId,
  );
  return {
    ...(await buildInstanceInputForDefinition(persona, runtime)),
    name: starter.name,
    teamId: WELCOME_TEAM_ID,
    relayUrl: relayUrl ?? undefined,
    spawnAfterCreate: false,
    startOnAppLaunch: false,
    respondTo: "owner-only",
  };
}

export function welcomeStarterRuntimeUpdate(
  existing: ManagedAgent,
  desired: CreateManagedAgentInput,
) {
  if (!desired.agentCommand) return null;

  const desiredArgs = desired.agentArgs ?? [];
  const desiredModel = desired.model ?? null;
  const desiredProvider = desired.provider ?? null;
  const desiredMcpCommand = desired.mcpCommand ?? "";
  if (
    existing.agentCommand === desired.agentCommand &&
    existing.agentArgs.join(",") === desiredArgs.join(",") &&
    existing.model === desiredModel &&
    existing.provider === desiredProvider &&
    existing.mcpCommand === desiredMcpCommand
  ) {
    return null;
  }

  return {
    pubkey: existing.pubkey,
    agentCommand: desired.agentCommand,
    harnessOverride: true,
    agentArgs: desiredArgs,
    mcpCommand: desiredMcpCommand,
    model: desiredModel,
    provider: desiredProvider,
  };
}

export function welcomeTeammateHasExpectedAccess(
  teammate: ManagedAgent,
  leadPubkey: string,
  agentAccessOwnerOnly: boolean,
) {
  if (agentAccessOwnerOnly) {
    // Welcome teammates are created owner-only, and the lead remains authorized
    // as a NIP-OA-verified sibling because every Welcome agent shares one owner.
    return (
      teammate.respondTo === "owner-only" &&
      teammate.respondToAllowlist.length === 0
    );
  }
  return (
    teammate.respondTo === "allowlist" &&
    teammate.respondToAllowlist.some(
      (pubkey) => normalizePubkey(pubkey) === normalizePubkey(leadPubkey),
    )
  );
}

/**
 * The access write that moves a Welcome teammate to the state this build
 * expects, or null when it is already there. The remediation target must track
 * {@link welcomeTeammateHasExpectedAccess}: writing `allowlist:[lead]` in an
 * owner-only build would fail the predicate again on the next provisioning
 * pass, so an upgraded install with pre-existing allowlisted teammates would
 * rewrite the same rejected state forever and keep restarting them.
 */
export function welcomeTeammateAccessUpdate(
  teammate: ManagedAgent,
  leadPubkey: string,
  agentAccessOwnerOnly: boolean,
): UpdateManagedAgentInput | null {
  if (
    welcomeTeammateHasExpectedAccess(teammate, leadPubkey, agentAccessOwnerOnly)
  ) {
    return null;
  }
  return agentAccessOwnerOnly
    ? {
        pubkey: teammate.pubkey,
        respondTo: "owner-only",
        respondToAllowlist: [],
      }
    : {
        pubkey: teammate.pubkey,
        respondTo: "allowlist",
        respondToAllowlist: [leadPubkey],
      };
}

/**
 * Ensure the complete built-in Welcome Team is ready for kickoff.
 * The team itself is Rust-seeded; this only activates personas, creates any
 * missing relay-scoped instances, and adds all three to Welcome as bots.
 */
async function provisionWelcomeTeam(
  channelId: string,
  relayUrl?: string | null,
): Promise<WelcomeTeamAgents> {
  const targetRelayUrl = normalizeRelayUrl(relayUrl);
  if (targetRelayUrl) {
    const identity = await getIdentity();
    // This must finish before the missing-agent check below. Otherwise a new
    // device can mint a lookalike while the real identity is still arriving.
    await backfillPersonaSync(identity.pubkey, targetRelayUrl);
  }
  const existingAgents = await removeDuplicateWelcomeTeamAgents(
    channelId,
    await listManagedAgents(),
    relayUrl,
  );
  await ensureWelcomeTeamPersonasActive();
  const [personas, runtimeCatalog, globalConfig, agentAccessOwnerOnly] =
    await Promise.all([
      listPersonas(),
      discoverAcpRuntimes(),
      getGlobalAgentConfig(),
      getAgentAccessOwnerOnly(),
    ]);
  const personasById = new Map(
    personas.map((persona) => [persona.id, persona]),
  );
  const runtimes = runtimeCatalog.filter(
    (runtime): runtime is AcpRuntime => runtime.availability === "available",
  );

  const agents: ManagedAgent[] = [];
  for (const starter of WELCOME_TEAM_STARTERS) {
    const persona = personasById.get(starter.personaId);
    if (!persona) {
      throw new Error(`${starter.name} agent not found.`);
    }
    const desired = await buildWelcomeStarterCreateInput(
      starter,
      persona,
      runtimes,
      globalConfig.preferred_runtime,
      relayUrl,
    );
    const existing = pickWelcomeTeamStarterAgentForRelay(
      existingAgents,
      starter,
      relayUrl,
    );
    if (existing) {
      const runtimeUpdate = welcomeStarterRuntimeUpdate(existing, desired);
      agents.push(
        runtimeUpdate
          ? (await updateManagedAgent(runtimeUpdate)).agent
          : existing,
      );
      continue;
    }

    const created = await createManagedAgent(desired);
    agents.push(created.agent);
  }
  const [lead, honey, pollen] = agents;
  if (!lead || !honey || !pollen) {
    throw new Error("Welcome Team provisioning did not return every starter.");
  }
  const welcomeAgents: WelcomeTeamAgents = [lead, honey, pollen];
  const leadPubkey = lead.pubkey;
  for (const index of [1, 2] as const) {
    const teammate = welcomeAgents[index];
    const accessUpdate = welcomeTeammateAccessUpdate(
      teammate,
      leadPubkey,
      agentAccessOwnerOnly,
    );
    if (accessUpdate) {
      const updated = await updateManagedAgent(accessUpdate);
      welcomeAgents[index] = updated.agent;
    }
  }
  await ensureWelcomeTeamMembership(channelId, welcomeAgents);
  return welcomeAgents;
}

export function ensureWelcomeTeam(
  channelId: string,
  relayUrl?: string | null,
): Promise<WelcomeTeamAgents> {
  const key = `${normalizeRelayUrl(relayUrl) ?? ""}:${channelId}`;
  const current = welcomeTeamPromises.get(key);
  if (current) return current;

  const promise = provisionWelcomeTeam(channelId, relayUrl).finally(() =>
    welcomeTeamPromises.delete(key),
  );
  welcomeTeamPromises.set(key, promise);
  return promise;
}
