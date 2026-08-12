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
    if (!device.online) continue;
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

  const currentDevice = devices.find(
    (device) => device.current && device.online,
  );
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

/** Whether the relay already has an online representative for this identity. */
export function hasOnlineAgentRepresentative(
  devices: readonly LoggedInDevice[],
  pubkey: string,
): boolean {
  const expected = normalizedPubkey(pubkey);
  return devices.some(
    (device) =>
      device.online &&
      device.activeAgents.some(
        (activePubkey) => normalizedPubkey(activePubkey) === expected,
      ),
  );
}

/** Optimistically remove a stopped identity from this device's relay snapshot. */
export function removeAgentFromCurrentDeviceSnapshot(
  devices: readonly LoggedInDevice[],
  pubkey: string,
): LoggedInDevice[] {
  const expected = normalizedPubkey(pubkey);
  return devices.map((device) =>
    device.current
      ? {
          ...device,
          activeAgents: device.activeAgents.filter(
            (activePubkey) => normalizedPubkey(activePubkey) !== expected,
          ),
          standbyAgents: device.standbyAgents.filter(
            (standbyPubkey) => normalizedPubkey(standbyPubkey) !== expected,
          ),
        }
      : device,
  );
}

export function findAgentExecutionLocation(
  locations: ReadonlyMap<string, AgentExecutionLocation>,
  pubkey: string,
): AgentExecutionLocation | undefined {
  return locations.get(normalizedPubkey(pubkey));
}
