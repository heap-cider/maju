import { MessageSquareText } from "lucide-react";
import * as React from "react";

import { useProfileQuery, useUsersBatchQuery } from "@/features/profile/hooks";
import { useMyRelayMembershipQuery } from "@/features/community-members/hooks";
import { canModerateCommunityContent } from "@/features/moderation/lib/contentModeration";
import { mergeCurrentProfileIntoLookup } from "@/features/profile/lib/identity";
import { getMentionTagPubkey } from "@/shared/lib/resolveMentionNames";
import type { Channel, ForumPost, ThreadReply } from "@/shared/api/types";
import { channelChrome } from "@/shared/layout/chromeLayout";
import { cn } from "@/shared/lib/cn";
import { Skeleton } from "@/shared/ui/skeleton";
import { VirtualizedList } from "@/shared/ui/VirtualizedList";

import {
  useCreateForumPostMutation,
  useCreateForumReplyMutation,
  useDeleteForumPostMutation,
  useDeleteForumReplyMutation,
  useEditForumContentMutation,
  useForumPostsQuery,
  useForumThreadQuery,
} from "../hooks";
import { ForumComposer } from "./ForumComposer";
import { EditForumContentDialog } from "./EditForumContentDialog";
import { ForumPostCard } from "./ForumPostCard";
import { ForumThreadPanel } from "./ForumThreadPanel";

type ForumViewProps = {
  channel: Channel;
  currentPubkey?: string;
  onClosePost: () => void;
  onSelectPost: (postId: string) => void;
  onTargetReached?: (messageId: string) => void;
  selectedPostId: string | null;
  targetReplyId: string | null;
};

type EditableForumContent = {
  content: string;
  eventId: string;
  label: "post" | "reply";
  tags: string[][];
};

export function isForumContentAuthor(
  postPubkey: string,
  currentPubkey?: string,
): boolean {
  if (!currentPubkey) return false;
  return postPubkey.toLowerCase() === currentPubkey.toLowerCase();
}

export function ForumView({
  channel,
  currentPubkey,
  onClosePost,
  onSelectPost,
  onTargetReached,
  selectedPostId,
  targetReplyId,
}: ForumViewProps) {
  const [isComposerOpen, setIsComposerOpen] = React.useState(false);
  const [editingContent, setEditingContent] =
    React.useState<EditableForumContent | null>(null);
  const postsScrollRef = React.useRef<HTMLDivElement>(null);

  const profileQuery = useProfileQuery();
  const relayMembershipQuery = useMyRelayMembershipQuery();
  const canModerateContent = canModerateCommunityContent(
    relayMembershipQuery.data?.role,
  );
  const postsQuery = useForumPostsQuery(channel);
  const threadQuery = useForumThreadQuery(
    selectedPostId ? channel.id : null,
    selectedPostId,
  );
  const createPostMutation = useCreateForumPostMutation(channel);
  const createReplyMutation = useCreateForumReplyMutation(channel);
  const deletePostMutation = useDeleteForumPostMutation(channel);
  const deleteReplyMutation = useDeleteForumReplyMutation(
    channel,
    selectedPostId,
  );
  const editContentMutation = useEditForumContentMutation(channel);

  const posts = postsQuery.data?.posts ?? [];

  // Collect all pubkeys from posts and thread for profile resolution.
  // Mentioned pubkeys (`p`/`mention` tags) must be included too: mention
  // chips resolve names from this same lookup, and a mentioned user who
  // never authored a post would otherwise render as a dead chip.
  const allPubkeys = React.useMemo(() => {
    const pubkeys = new Set<string>();
    const addMentionPubkeys = (tags?: string[][]) => {
      for (const tag of tags ?? []) {
        const pubkey = getMentionTagPubkey(tag);
        if (pubkey) {
          pubkeys.add(pubkey);
        }
      }
    };
    for (const post of posts) {
      pubkeys.add(post.pubkey);
      addMentionPubkeys(post.tags);
      if (post.threadSummary?.participants) {
        for (const pk of post.threadSummary.participants) {
          pubkeys.add(pk);
        }
      }
    }
    if (threadQuery.data) {
      pubkeys.add(threadQuery.data.post.pubkey);
      addMentionPubkeys(threadQuery.data.post.tags);
      for (const reply of threadQuery.data.replies) {
        pubkeys.add(reply.pubkey);
        addMentionPubkeys(reply.tags);
      }
    }
    return [...pubkeys];
  }, [posts, threadQuery.data]);

  const profilesQuery = useUsersBatchQuery(allPubkeys, {
    enabled: allPubkeys.length > 0,
  });
  const effectiveCurrentPubkey = currentPubkey ?? profileQuery.data?.pubkey;
  const profiles = React.useMemo(
    () =>
      mergeCurrentProfileIntoLookup(
        profilesQuery.data?.profiles,
        profileQuery.data,
      ),
    [profileQuery.data, profilesQuery.data?.profiles],
  );

  const previousChannelIdRef = React.useRef(channel.id);
  React.useEffect(() => {
    if (previousChannelIdRef.current === channel.id) {
      return;
    }

    previousChannelIdRef.current = channel.id;
    setIsComposerOpen(false);
    setEditingContent(null);
  }, [channel.id]);

  const openPostEditor = React.useCallback((post: ForumPost) => {
    setEditingContent({
      content: post.content,
      eventId: post.eventId,
      label: "post",
      tags: post.tags,
    });
  }, []);
  const openReplyEditor = React.useCallback((reply: ThreadReply) => {
    setEditingContent({
      content: reply.content,
      eventId: reply.eventId,
      label: "reply",
      tags: reply.tags,
    });
  }, []);
  const editDialog = editingContent ? (
    <EditForumContentDialog
      content={editingContent.content}
      isSaving={editContentMutation.isPending}
      label={editingContent.label}
      onOpenChange={(open) => {
        if (!open) setEditingContent(null);
      }}
      onSave={(content) =>
        editContentMutation.mutateAsync({
          content,
          eventId: editingContent.eventId,
          tags: editingContent.tags,
        })
      }
      open
    />
  ) : null;

  if (selectedPostId) {
    const threadPost = threadQuery.data?.post;
    const canDeleteExpandedPost = threadPost
      ? isForumContentAuthor(threadPost.pubkey, effectiveCurrentPubkey) ||
        canModerateContent
      : false;

    return (
      <>
        <ForumThreadPanel
          canDeletePost={canDeleteExpandedPost}
          canModerateContent={canModerateContent}
          currentPubkey={effectiveCurrentPubkey}
          isDeletingPost={deletePostMutation.isPending}
          isLoading={threadQuery.isLoading}
          isSendingReply={createReplyMutation.isPending}
          onBack={onClosePost}
          onDeletePost={(eventId) => {
            deletePostMutation.mutate(
              {
                eventId,
                moderatorDelete:
                  threadPost != null &&
                  !isForumContentAuthor(
                    threadPost.pubkey,
                    effectiveCurrentPubkey,
                  ),
              },
              { onSuccess: onClosePost },
            );
          }}
          onDeleteReply={(eventId) => {
            const reply = threadQuery.data?.replies.find(
              (candidate) => candidate.eventId === eventId,
            );
            deleteReplyMutation.mutate({
              eventId,
              moderatorDelete:
                reply != null &&
                !isForumContentAuthor(reply.pubkey, effectiveCurrentPubkey),
            });
          }}
          onEditPost={canDeleteExpandedPost ? openPostEditor : undefined}
          onEditReply={openReplyEditor}
          channelId={channel.id}
          onReply={(content, mentionPubkeys, mediaTags) =>
            createReplyMutation.mutateAsync({
              content,
              parentEventId: selectedPostId,
              mentionPubkeys,
              mediaTags,
            })
          }
          onTargetReached={onTargetReached}
          profiles={profiles}
          targetEventId={targetReplyId}
          thread={threadQuery.data}
        />
        {editDialog}
      </>
    );
  }

  return (
    <div className={cn("flex h-full flex-col", channelChrome.contentPadding)}>
      <div className="border-b border-border/60 p-4">
        {isComposerOpen ? (
          <ForumComposer
            autocompleteBelow
            channelId={channel.id}
            isSending={createPostMutation.isPending}
            onCancel={() => setIsComposerOpen(false)}
            onSubmit={async (content, mentionPubkeys, mediaTags) => {
              await createPostMutation.mutateAsync({
                content,
                mentionPubkeys,
                mediaTags,
              });
              setIsComposerOpen(false);
            }}
            placeholder="Write your post..."
            profiles={profiles}
          />
        ) : (
          <button
            className="w-full rounded-xl border border-dashed border-border/80 px-4 py-3 text-left text-sm text-muted-foreground transition-colors hover:border-border hover:bg-accent/30 hover:text-foreground"
            disabled={!channel.isMember || channel.archivedAt !== null}
            onClick={() => setIsComposerOpen(true)}
            type="button"
          >
            {channel.archivedAt
              ? "This forum is archived."
              : !channel.isMember
                ? "Join this forum to create posts."
                : "Start a new post..."}
          </button>
        )}
      </div>

      <div
        className="flex-1 overflow-y-auto"
        data-scroll-restoration-id={`forum-list:${channel.id}`}
        ref={postsScrollRef}
      >
        {postsQuery.isLoading ? (
          <div className="space-y-3 p-4">
            <Skeleton className="h-24 w-full rounded-xl" />
            <Skeleton className="h-24 w-full rounded-xl" />
            <Skeleton className="h-24 w-full rounded-xl" />
          </div>
        ) : posts.length === 0 ? (
          <div className="flex flex-col items-center justify-center gap-3 px-4 py-16 text-center">
            <MessageSquareText className="h-10 w-10 text-muted-foreground/40" />
            <div>
              <p className="text-sm font-medium text-foreground/70">
                No posts yet
              </p>
              <p className="mt-1 text-xs text-muted-foreground">
                Start a discussion by creating the first post.
              </p>
            </div>
          </div>
        ) : (
          <VirtualizedList
            estimateSize={120}
            getItemKey={(post) => post.eventId}
            innerClassName="p-4"
            items={posts}
            renderItem={(post) => (
              <div className="pb-3">
                <ForumPostCard
                  canDelete={
                    isForumContentAuthor(post.pubkey, effectiveCurrentPubkey) ||
                    canModerateContent
                  }
                  canEdit={
                    isForumContentAuthor(post.pubkey, effectiveCurrentPubkey) ||
                    canModerateContent
                  }
                  currentPubkey={effectiveCurrentPubkey}
                  isActive={selectedPostId === post.eventId}
                  isDeleting={
                    deletePostMutation.isPending &&
                    deletePostMutation.variables?.eventId === post.eventId
                  }
                  onClick={() => onSelectPost(post.eventId)}
                  onDelete={(eventId) => {
                    deletePostMutation.mutate({
                      eventId,
                      moderatorDelete: !isForumContentAuthor(
                        post.pubkey,
                        effectiveCurrentPubkey,
                      ),
                    });
                  }}
                  onEdit={openPostEditor}
                  post={post}
                  profiles={profiles}
                />
              </div>
            )}
            scrollRef={postsScrollRef}
          />
        )}
      </div>
      {editDialog}
    </div>
  );
}
