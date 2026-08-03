import { applyEditTagOverlay } from "@/features/messages/lib/applyEditTagOverlay.mjs";
import type {
  ForumPost,
  ForumPostsResponse,
  ForumThreadResponse,
  RelayEvent,
  ThreadReply,
} from "@/shared/api/types";
import {
  KIND_DELETION,
  KIND_NIP29_DELETE_EVENT,
  KIND_STREAM_MESSAGE_EDIT,
} from "@/shared/constants/kinds";

type ForumContent = {
  eventId: string;
  content: string;
  tags: string[][];
  editedByPubkey?: string;
};

export type ForumPostWithEdits = ForumPost & { editedByPubkey?: string };
export type ForumReplyWithEdits = ThreadReply & { editedByPubkey?: string };
export type ForumPostsWithEditsResponse = Omit<ForumPostsResponse, "posts"> & {
  posts: ForumPostWithEdits[];
};
export type ForumThreadWithEditsResponse = Omit<
  ForumThreadResponse,
  "post" | "replies"
> & { post: ForumPostWithEdits; replies: ForumReplyWithEdits[] };

function referencedEventIds(tags: string[][]) {
  return tags.flatMap((tag) => (tag[0] === "e" && tag[1] ? [tag[1]] : []));
}

/** Apply the same edit/delete overlay used by channel timelines to forum data. */
export function applyForumStructuralEvents<T extends ForumContent>(
  content: T[],
  structuralEvents: RelayEvent[],
): Array<T & { editedByPubkey?: string }> {
  const deletedIds = new Set(
    structuralEvents
      .filter(
        (event) =>
          event.kind === KIND_DELETION ||
          event.kind === KIND_NIP29_DELETE_EVENT,
      )
      .flatMap((event) => referencedEventIds(event.tags)),
  );
  const latestEditByTarget = new Map<string, RelayEvent>();

  for (const event of structuralEvents) {
    if (event.kind !== KIND_STREAM_MESSAGE_EDIT || deletedIds.has(event.id)) {
      continue;
    }
    const targetId = referencedEventIds(event.tags)[0];
    if (!targetId || deletedIds.has(targetId)) continue;
    const current = latestEditByTarget.get(targetId);
    if (
      !current ||
      event.created_at > current.created_at ||
      (event.created_at === current.created_at && event.id > current.id)
    ) {
      latestEditByTarget.set(targetId, event);
    }
  }

  return content
    .filter((item) => !deletedIds.has(item.eventId))
    .map((item) => {
      const edit = latestEditByTarget.get(item.eventId);
      return edit
        ? {
            ...item,
            content: edit.content,
            editedByPubkey: edit.pubkey.toLowerCase(),
            tags: applyEditTagOverlay(item.tags, edit.tags),
          }
        : item;
    });
}
