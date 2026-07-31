import { RecoveryScreen } from "./RecoveryScreen";

export function RelaunchRequiredScreen() {
  return (
    <RecoveryScreen
      testId="relaunch-required"
      title="Restart Maju to finish recovery"
      body="Your identity was updated. Maju needs to restart so syncing and agents run under it."
    />
  );
}
