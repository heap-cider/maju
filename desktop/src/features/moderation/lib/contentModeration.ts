import type { RelayMemberRole } from "@/shared/api/types";

/** Community roles whose signed kind:9005 events may remove any channel item. */
export function canModerateCommunityContent(
  role: RelayMemberRole | null | undefined,
): boolean {
  return role === "owner" || role === "admin";
}
