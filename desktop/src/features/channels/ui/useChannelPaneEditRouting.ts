import * as React from "react";
import { toast } from "sonner";

import type { ChannelPaneProps } from "@/features/channels/ui/ChannelPane.types";
import type { MainTimelineEntry } from "@/features/messages/lib/threadPanel";
import { isThreadReply } from "@/features/messages/lib/threading";
import type { TimelineMessage } from "@/features/messages/types";
import { KIND_SYSTEM_MESSAGE } from "@/shared/constants/kinds";

type ChannelPaneEditRoutingOptions = Pick<
  ChannelPaneProps,
  | "currentPubkey"
  | "editTarget"
  | "isSinglePanelView"
  | "onCloseThread"
  | "onEdit"
  | "threadHeadMessage"
  | "threadMessages"
> & {
  activeChannelId: string | null;
  channelIsCovered: boolean;
  mainTimelineEntries: readonly MainTimelineEntry[];
  useFocusThreadDrawer: boolean;
};

export function useChannelPaneEditRouting({
  activeChannelId,
  channelIsCovered,
  currentPubkey,
  editTarget,
  isSinglePanelView = false,
  mainTimelineEntries,
  onCloseThread,
  onEdit,
  threadHeadMessage,
  threadMessages,
  useFocusThreadDrawer,
}: ChannelPaneEditRoutingOptions) {
  const findLastOwnEditable = React.useCallback(
    (candidates: readonly TimelineMessage[]): TimelineMessage | null => {
      if (!onEdit || !currentPubkey) return null;
      let best: TimelineMessage | null = null;
      for (const message of candidates) {
        if (
          message.kind === KIND_SYSTEM_MESSAGE ||
          message.pubkey !== currentPubkey ||
          message.pending
        ) {
          continue;
        }
        if (!best || message.createdAt >= best.createdAt) {
          best = message;
        }
      }
      return best;
    },
    [onEdit, currentPubkey],
  );
  const pendingMainEditRef = React.useRef<TimelineMessage | null>(null);
  const editTargetRef = React.useRef(editTarget);
  editTargetRef.current = editTarget;
  const pendingMainEditContextRef = React.useRef({
    channelId: activeChannelId,
    threadId: threadHeadMessage?.id ?? null,
  });
  const pendingMainEditContext = {
    channelId: activeChannelId,
    threadId: threadHeadMessage?.id ?? null,
  };
  const previousPendingContext = pendingMainEditContextRef.current;
  if (
    previousPendingContext.channelId !== pendingMainEditContext.channelId ||
    (previousPendingContext.threadId !== null &&
      pendingMainEditContext.threadId !== null &&
      previousPendingContext.threadId !== pendingMainEditContext.threadId)
  ) {
    pendingMainEditRef.current = null;
  }
  pendingMainEditContextRef.current = pendingMainEditContext;

  const handleRoutedEdit = React.useCallback(
    (message: TimelineMessage): boolean => {
      const currentEditTarget = editTargetRef.current;
      if (
        currentEditTarget &&
        currentEditTarget.id !== message.id &&
        currentEditTarget.isThreadReply !== isThreadReply(message.tags ?? [])
      ) {
        pendingMainEditRef.current = null;
        toast.info("Finish or cancel your edit first.");
        return false;
      }
      if (currentEditTarget?.id === message.id) {
        pendingMainEditRef.current = null;
        onEdit?.(message);
        return true;
      }
      if (
        !isThreadReply(message.tags ?? []) &&
        (isSinglePanelView || useFocusThreadDrawer)
      ) {
        pendingMainEditRef.current = message;
        onCloseThread();
        return true;
      }
      onEdit?.(message);
      return Boolean(onEdit);
    },
    [isSinglePanelView, onCloseThread, onEdit, useFocusThreadDrawer],
  );
  const handleEditLastOwnMainMessage = React.useCallback((): boolean => {
    const target = findLastOwnEditable(
      mainTimelineEntries.map((entry) => entry.message),
    );
    return target ? handleRoutedEdit(target) : false;
  }, [findLastOwnEditable, handleRoutedEdit, mainTimelineEntries]);
  const handleEditLastOwnThreadMessage = React.useCallback((): boolean => {
    const scope: TimelineMessage[] = [];
    if (threadHeadMessage) scope.push(threadHeadMessage);
    for (const entry of threadMessages) scope.push(entry.message);
    const target = findLastOwnEditable(scope);
    return target ? handleRoutedEdit(target) : false;
  }, [
    findLastOwnEditable,
    handleRoutedEdit,
    threadHeadMessage,
    threadMessages,
  ]);

  React.useEffect(() => {
    const pendingMainEdit = pendingMainEditRef.current;
    if (!pendingMainEdit || isSinglePanelView || channelIsCovered) return;
    pendingMainEditRef.current = null;
    onEdit?.(pendingMainEdit);
  }, [channelIsCovered, isSinglePanelView, onEdit]);

  return {
    handleEditLastOwnMainMessage,
    handleEditLastOwnThreadMessage,
    handleRoutedEdit,
  };
}
