import { useQueryClient } from "@tanstack/react-query";
import * as React from "react";

import { hasPersistedHydratedChannel } from "@/features/messages/lib/channelHeadCache";
import {
  resolveTimelineLoadingLatch,
  selectTimelineLoadingState,
  type TimelineQueryStatus,
} from "@/features/messages/lib/timelineLoadingState";
import type { Channel } from "@/shared/api/types";

type ChannelTimelineLoadingOptions = {
  activeChannel: Channel | null;
  activeChannelId: string | null;
  queryStatus: TimelineQueryStatus;
};

export function useChannelTimelineLoading({
  activeChannel,
  activeChannelId,
  queryStatus,
}: ChannelTimelineLoadingOptions): boolean {
  const queryClient = useQueryClient();
  const settledChannelIdRef = React.useRef<string | null>(null);
  const hasSettledThisChannel =
    activeChannelId !== null && settledChannelIdRef.current === activeChannelId;
  const loadingNow =
    activeChannel !== null &&
    activeChannel.channelType !== "forum" &&
    selectTimelineLoadingState(
      queryStatus,
      hasSettledThisChannel ||
        (activeChannelId !== null &&
          hasPersistedHydratedChannel(queryClient, activeChannelId)),
    );
  const { settledChannelId, isLoading } = resolveTimelineLoadingLatch(
    settledChannelIdRef.current,
    activeChannelId,
    loadingNow,
  );
  settledChannelIdRef.current = settledChannelId;
  return isLoading;
}
