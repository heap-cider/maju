import type * as React from "react";
import type { Editor } from "@tiptap/core";
import type {
  ChannelSuggestion,
  UseChannelLinksResult,
} from "../lib/useChannelLinks";
import type {
  EmojiSuggestion,
  UseEmojiAutocompleteResult,
} from "../lib/useEmojiAutocomplete";
import type { UseMentionsResult } from "../lib/useMentions";
import { isMentionCodeContext } from "../lib/mentionCodeContext";
import {
  focusMentionOptionsTrigger,
  type MentionSuggestion,
} from "./MentionAutocomplete";

/** Give autocomplete first refusal before link-card and edit-mode shortcuts. */
export function handleComposerAutocompleteKeyDown(
  event: React.KeyboardEvent<HTMLDivElement>,
  {
    emojiAutocomplete,
    channelLinks,
    mentions,
    editor,
    formElement,
    applyEmojiInsert,
    applyChannelInsert,
    selectMentionSuggestion,
  }: {
    emojiAutocomplete: Pick<UseEmojiAutocompleteResult, "handleEmojiKeyDown">;
    channelLinks: Pick<UseChannelLinksResult, "handleChannelKeyDown">;
    mentions: Pick<UseMentionsResult, "isMentionOpen" | "handleMentionKeyDown">;
    editor: Editor | null;
    formElement: HTMLFormElement | null;
    applyEmojiInsert: (suggestion: EmojiSuggestion) => void;
    applyChannelInsert: (suggestion: ChannelSuggestion) => void;
    selectMentionSuggestion: (suggestion: MentionSuggestion) => void;
  },
): boolean {
  const emojiResult = emojiAutocomplete.handleEmojiKeyDown(event);
  if (emojiResult.handled) {
    if (emojiResult.suggestion) applyEmojiInsert(emojiResult.suggestion);
    return true;
  }
  const channelResult = channelLinks.handleChannelKeyDown(event);
  if (channelResult.handled) {
    if (channelResult.suggestion) applyChannelInsert(channelResult.suggestion);
    return true;
  }
  // Shift+Tab enters the mention overlay's Options controls; forward Tab
  // selects the highlighted suggestion. Otherwise preserve native focus moves.
  if (
    event.key === "Tab" &&
    event.shiftKey &&
    mentions.isMentionOpen &&
    focusMentionOptionsTrigger(formElement)
  ) {
    event.preventDefault();
    return true;
  }
  const { handled, suggestion } = mentions.handleMentionKeyDown(event, {
    isCodeContext: () => isMentionCodeContext(editor),
  });
  if (handled && suggestion) selectMentionSuggestion(suggestion);
  return handled;
}
