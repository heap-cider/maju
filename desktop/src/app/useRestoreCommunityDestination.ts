import * as React from "react";
import type { useAppNavigation } from "@/app/navigation/useAppNavigation";
import {
  consumePendingCommunityRestore,
  loadCommunityDestination,
  saveCommunityDestination,
} from "@/features/communities/communityNavigationStorage";

type Navigation = ReturnType<typeof useAppNavigation>;

/** Restore an explicitly switched community only after its channel roster is ready. */
export function useRestoreCommunityDestination({
  activeCommunityId,
  channelsReady,
  channelsUpdatedAt,
  sidebarChannels,
  selectedView,
  goChannel,
  goHome,
}: {
  activeCommunityId: string | undefined;
  channelsReady: boolean;
  channelsUpdatedAt: number;
  sidebarChannels: ReadonlyArray<{ id: string }>;
  selectedView: string;
  goChannel: Navigation["goChannel"];
  goHome: Navigation["goHome"];
}) {
  const hasRestoredCommunityDestinationRef = React.useRef(false);
  React.useEffect(() => {
    if (
      hasRestoredCommunityDestinationRef.current ||
      !channelsReady ||
      channelsUpdatedAt === 0 ||
      !activeCommunityId
    ) {
      return;
    }
    hasRestoredCommunityDestinationRef.current = true;

    // Restoration belongs to an explicit community transition. Cold boot and
    // reconnect remounts must preserve the route the user explicitly opened.
    if (!consumePendingCommunityRestore(activeCommunityId)) {
      return;
    }

    const destination = loadCommunityDestination(activeCommunityId);
    if (!destination || destination.kind === "home") {
      return;
    }

    const channelIsAvailable = sidebarChannels.some(
      (channel) => channel.id === destination.channelId,
    );
    if (!channelIsAvailable) {
      saveCommunityDestination(activeCommunityId, { kind: "home" });
      void goHome({ replace: true });
      return;
    }

    // The normal switch path writes the remembered channel into the hash before
    // the target community mounts, so no intermediate Inbox frame is painted.
    // Older transition callers may still arrive at neutral Home; repair those.
    if (selectedView === "home") {
      void goChannel(destination.channelId, { replace: true });
    }
  }, [
    channelsUpdatedAt,
    channelsReady,
    activeCommunityId,
    goChannel,
    goHome,
    selectedView,
    sidebarChannels,
  ]);
}
