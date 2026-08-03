import { signOut } from "@/shared/api/tauriIdentity";

const DISCONNECTED_PREFIX = "blocked: device session disconnected";

export function beginDeviceSessionDisconnect(
  success: boolean,
  message: string,
): Error | null {
  if (success || !message.startsWith(DISCONNECTED_PREFIX)) return null;
  void signOut()
    .catch(() => undefined)
    .finally(() => {
      window.localStorage.clear();
      window.sessionStorage.clear();
    });
  return new Error(message);
}
