import { invokeTauri } from "@/shared/api/tauri";

export async function deleteMessage(
  channelId: string,
  eventId: string,
  moderatorDelete = false,
): Promise<void> {
  await invokeTauri("delete_message", {
    channelId,
    eventId,
    moderatorDelete,
  });
}
