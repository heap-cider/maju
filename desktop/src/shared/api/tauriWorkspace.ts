import { invokeTauri } from "@/shared/api/tauri";
import type { AppliedWorkspaceInfo } from "@/shared/api/workspaceTypes";

export async function applyCommunity(
  relayUrl: string,
  nsec?: string,
  token?: string,
  reposDir?: string,
  agentManagedProfiles?: boolean,
  threadScopedAcpSessions?: boolean,
): Promise<AppliedWorkspaceInfo> {
  return invokeTauri<AppliedWorkspaceInfo>("apply_workspace", {
    relayUrl,
    nsec: nsec ?? null,
    token: token ?? null,
    reposDir: reposDir ?? null,
    agentManagedProfiles: agentManagedProfiles ?? false,
    threadScopedAcpSessions: threadScopedAcpSessions ?? false,
  });
}

export const setAgentManagedProfiles = (enabled: boolean) =>
  invokeTauri("set_agent_managed_profiles", { enabled });

export const setThreadScopedAcpSessions = (enabled: boolean) =>
  invokeTauri("set_thread_scoped_acp_sessions", { enabled });
