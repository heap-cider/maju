import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { getForumPosts, getForumThread } from "@/shared/api/forum";
import { useRelaySelfQuery } from "@/features/moderation/hooks";
import { fetchStructuralAuxForMessages } from "@/features/messages/lib/auxBackfill";
import { applyForumStructuralEvents } from "@/features/forum/lib/applyForumStructuralEvents";
import type {
  ForumPostsWithEditsResponse,
  ForumThreadWithEditsResponse,
} from "@/features/forum/lib/applyForumStructuralEvents";
import {
  deleteMessage,
  editMessage,
  sendChannelMessage,
} from "@/shared/api/tauri";
import type { Channel } from "@/shared/api/types";
import { KIND_FORUM_COMMENT, KIND_FORUM_POST } from "@/shared/constants/kinds";

async function fetchForumStructuralEvents(
  channelId: string,
  eventIds: string[],
) {
  try {
    return await fetchStructuralAuxForMessages(channelId, eventIds);
  } catch (error) {
    console.error("Failed to load forum edits", channelId, error);
    return [];
  }
}

export function forumPostsQueryKey(channelId: string) {
  return ["forum-posts", channelId] as const;
}

export function forumThreadQueryKey(channelId: string, eventId: string) {
  return ["forum-thread", channelId, eventId] as const;
}

export function useForumPostsQuery(channel: Channel | null) {
  const channelId = channel?.id ?? "";
  const enabled = channel !== null && channel.channelType === "forum";
  const relaySelfPubkey = useRelaySelfQuery(enabled).data;

  return useQuery<ForumPostsWithEditsResponse>({
    enabled,
    queryKey: [...forumPostsQueryKey(channelId), relaySelfPubkey ?? null],
    queryFn: async () => {
      const response = await getForumPosts(
        channelId,
        50,
        undefined,
        relaySelfPubkey,
      );
      const structuralEvents = await fetchForumStructuralEvents(
        channelId,
        response.posts.map((post) => post.eventId),
      );
      return {
        ...response,
        posts: applyForumStructuralEvents(response.posts, structuralEvents),
      };
    },
    staleTime: 15_000,
    refetchInterval: 15_000,
  });
}

export function useForumThreadQuery(
  channelId: string | null,
  eventId: string | null,
) {
  const enabled = channelId !== null && eventId !== null;
  const relaySelfPubkey = useRelaySelfQuery(enabled).data;

  return useQuery<ForumThreadWithEditsResponse>({
    enabled,
    queryKey: [
      ...forumThreadQueryKey(channelId ?? "", eventId ?? ""),
      relaySelfPubkey ?? null,
    ],
    queryFn: async () => {
      const response = await getForumThread(
        channelId ?? "",
        eventId ?? "",
        undefined,
        undefined,
        relaySelfPubkey,
      );
      const structuralEvents = await fetchForumStructuralEvents(
        channelId ?? "",
        [
          response.post.eventId,
          ...response.replies.map((reply) => reply.eventId),
        ],
      );
      const post = applyForumStructuralEvents(
        [response.post],
        structuralEvents,
      )[0];
      return {
        ...response,
        post: post ?? response.post,
        replies: applyForumStructuralEvents(response.replies, structuralEvents),
      };
    },
    staleTime: 10_000,
    refetchInterval: 10_000,
  });
}

export function useEditForumContentMutation(channel: Channel | null) {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({
      eventId,
      content,
      tags,
    }: {
      eventId: string;
      content: string;
      tags: string[][];
    }) => {
      if (!channel) throw new Error("No channel selected.");
      await editMessage(
        channel.id,
        eventId,
        content,
        tags.filter((tag) => tag[0] === "imeta"),
        tags.filter((tag) => tag[0] === "emoji"),
      );
    },
    onSuccess: () => {
      if (!channel) return;
      void queryClient.invalidateQueries({
        queryKey: forumPostsQueryKey(channel.id),
      });
      void queryClient.invalidateQueries({
        queryKey: ["forum-thread", channel.id],
      });
    },
  });
}

export function useCreateForumPostMutation(channel: Channel | null) {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({
      content,
      mentionPubkeys,
      mediaTags,
    }: {
      content: string;
      mentionPubkeys?: string[];
      mediaTags?: string[][];
    }) => {
      if (!channel) {
        throw new Error("No channel selected.");
      }

      return sendChannelMessage(
        channel.id,
        content,
        null,
        mediaTags,
        mentionPubkeys,
        KIND_FORUM_POST,
      );
    },
    onSuccess: () => {
      if (channel) {
        void queryClient.invalidateQueries({
          queryKey: forumPostsQueryKey(channel.id),
        });
      }
    },
  });
}

export function useDeleteForumPostMutation(channel: Channel | null) {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({
      eventId,
      moderatorDelete = false,
    }: {
      eventId: string;
      moderatorDelete?: boolean;
    }) => {
      if (!channel) {
        throw new Error("No channel selected.");
      }
      await deleteMessage(channel.id, eventId, moderatorDelete);
    },
    onSuccess: () => {
      if (channel) {
        void queryClient.invalidateQueries({
          queryKey: forumPostsQueryKey(channel.id),
        });
      }
    },
  });
}

export function useDeleteForumReplyMutation(
  channel: Channel | null,
  rootEventId: string | null,
) {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({
      eventId,
      moderatorDelete = false,
    }: {
      eventId: string;
      moderatorDelete?: boolean;
    }) => {
      if (!channel) {
        throw new Error("No channel selected.");
      }
      await deleteMessage(channel.id, eventId, moderatorDelete);
    },
    onSuccess: () => {
      if (channel) {
        if (rootEventId) {
          void queryClient.invalidateQueries({
            queryKey: forumThreadQueryKey(channel.id, rootEventId),
          });
        }
        void queryClient.invalidateQueries({
          queryKey: forumPostsQueryKey(channel.id),
        });
      }
    },
  });
}

export function useCreateForumReplyMutation(channel: Channel | null) {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({
      content,
      parentEventId,
      mentionPubkeys,
      mediaTags,
    }: {
      content: string;
      parentEventId: string;
      mentionPubkeys?: string[];
      mediaTags?: string[][];
    }) => {
      if (!channel) {
        throw new Error("No channel selected.");
      }

      return sendChannelMessage(
        channel.id,
        content,
        parentEventId,
        mediaTags,
        mentionPubkeys,
        KIND_FORUM_COMMENT,
      );
    },
    onSuccess: (_data, variables) => {
      if (channel) {
        void queryClient.invalidateQueries({
          queryKey: forumThreadQueryKey(channel.id, variables.parentEventId),
        });
        void queryClient.invalidateQueries({
          queryKey: forumPostsQueryKey(channel.id),
        });
      }
    },
  });
}
