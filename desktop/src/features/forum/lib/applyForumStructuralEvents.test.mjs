import assert from "node:assert/strict";
import test from "node:test";

import { applyForumStructuralEvents } from "./applyForumStructuralEvents.ts";

const content = (eventId, body = "original") => ({
  eventId,
  content: body,
  tags: [
    ["h", "forum"],
    ["imeta", "url https://old.example/file"],
  ],
});
const event = (id, kind, createdAt, targetId, body = "") => ({
  id,
  pubkey: "A".repeat(64),
  created_at: createdAt,
  kind,
  tags: [
    ["h", "forum"],
    ["e", targetId],
  ],
  content: body,
  sig: "b".repeat(128),
});

test("newest authorized forum edit wins and keeps editor provenance", () => {
  const result = applyForumStructuralEvents(
    [content("post")],
    [
      event("edit-a", 40003, 10, "post", "older"),
      event("edit-b", 40003, 11, "post", "newer"),
    ],
  );
  assert.equal(result[0].content, "newer");
  assert.equal(result[0].editedByPubkey, "a".repeat(64));
});

test("deleting a forum item hides it and deleting an edit restores the original", () => {
  const edit = event("edit", 40003, 10, "reply", "changed");
  assert.equal(
    applyForumStructuralEvents(
      [content("reply")],
      [edit, event("delete-edit", 5, 11, edit.id)],
    )[0].content,
    "original",
  );
  assert.deepEqual(
    applyForumStructuralEvents(
      [content("reply")],
      [event("delete-reply", 9005, 12, "reply")],
    ),
    [],
  );
});
