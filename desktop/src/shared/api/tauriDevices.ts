import { invoke } from "@tauri-apps/api/core";

type RawDeviceStatus = {
  device_id: string;
  session_id: string;
  name: string;
  platform: string;
  app_version: string;
  last_seen: number;
  online: boolean;
  current: boolean;
  active_agents: string[];
  standby_agents: string[];
};

export type LoggedInDevice = {
  deviceId: string;
  sessionId: string;
  name: string;
  platform: string;
  appVersion: string;
  lastSeen: number;
  online: boolean;
  current: boolean;
  activeAgents: string[];
  standbyAgents: string[];
};

function fromRaw(raw: RawDeviceStatus): LoggedInDevice {
  return {
    deviceId: raw.device_id,
    sessionId: raw.session_id,
    name: raw.name,
    platform: raw.platform,
    appVersion: raw.app_version,
    lastSeen: raw.last_seen,
    online: raw.online,
    current: raw.current,
    activeAgents: raw.active_agents,
    standbyAgents: raw.standby_agents,
  };
}

export async function listLoggedInDevices(): Promise<LoggedInDevice[]> {
  const rows = await invoke<RawDeviceStatus[]>("list_logged_in_devices");
  return rows.map(fromRaw);
}

export async function renameCurrentDevice(name: string): Promise<void> {
  await invoke("rename_current_device", { name });
}

export async function disconnectLoggedInDevice(
  device: Pick<LoggedInDevice, "deviceId" | "sessionId">,
): Promise<void> {
  await invoke("disconnect_logged_in_device", {
    deviceId: device.deviceId,
    sessionId: device.sessionId,
  });
}
