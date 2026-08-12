import type { QueryClient, QueryKey } from "@tanstack/react-query";

import type { ManagedAgent } from "@/shared/api/types";
import type { LoggedInDevice } from "@/shared/api/tauriDevices";
import { removeAgentFromCurrentDeviceSnapshot } from "@/features/agents/agentExecutionLocations";

export function updateCachedManagedAgent(
  queryClient: QueryClient,
  queryKey: QueryKey,
  updated: ManagedAgent,
) {
  queryClient.setQueryData<ManagedAgent[]>(queryKey, (current) => {
    if (!current) return current;
    return current.map((agent) =>
      agent.pubkey === updated.pubkey ? updated : agent,
    );
  });
}

export function removeCachedCurrentDeviceAgent(
  queryClient: QueryClient,
  queryKey: QueryKey,
  pubkey: string,
) {
  queryClient.setQueryData<LoggedInDevice[]>(queryKey, (current) =>
    current ? removeAgentFromCurrentDeviceSnapshot(current, pubkey) : current,
  );
}
