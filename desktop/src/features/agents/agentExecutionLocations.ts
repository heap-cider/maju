import type { LoggedInDevice } from "@/shared/api/tauriDevices";

export type AgentExecutionLocation = {
  representativeDeviceName: string | null;
  representativeIsCurrent: boolean;
  currentDeviceIsStandby: boolean;
};

function normalizedPubkey(pubkey: string): string {
  return pubkey.trim().toLowerCase();
}

/**
 * Projects the relay's device/runner lease snapshot into one entry per agent.
 * The relay is authoritative here: a process error on this PC must not make an
 * agent look offline while its representative is healthy on another PC.
 */
export function indexAgentExecutionLocations(
  devices: readonly LoggedInDevice[],
): ReadonlyMap<string, AgentExecutionLocation> {
  const locations = new Map<string, AgentExecutionLocation>();

  for (const device of devices) {
    for (const pubkey of device.activeAgents) {
      const key = normalizedPubkey(pubkey);
      const previous = locations.get(key);
      if (!previous?.representativeDeviceName || device.current) {
        locations.set(key, {
          representativeDeviceName: device.name,
          representativeIsCurrent: device.current,
          currentDeviceIsStandby: previous?.currentDeviceIsStandby ?? false,
        });
      }
    }
  }

  const currentDevice = devices.find((device) => device.current);
  if (currentDevice) {
    for (const pubkey of currentDevice.standbyAgents) {
      const key = normalizedPubkey(pubkey);
      const previous = locations.get(key);
      locations.set(key, {
        representativeDeviceName: previous?.representativeDeviceName ?? null,
        representativeIsCurrent: previous?.representativeIsCurrent ?? false,
        currentDeviceIsStandby: true,
      });
    }
  }

  return locations;
}

export function findAgentExecutionLocation(
  locations: ReadonlyMap<string, AgentExecutionLocation>,
  pubkey: string,
): AgentExecutionLocation | undefined {
  return locations.get(normalizedPubkey(pubkey));
}
