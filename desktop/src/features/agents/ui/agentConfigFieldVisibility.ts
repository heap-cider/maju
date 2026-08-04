export type AgentConfigDisclosure =
  | "full"
  | "onboarding-essential"
  | "progressive-defaults";

export function resolveDisclosure(disclosure: AgentConfigDisclosure) {
  const full = disclosure !== "onboarding-essential";
  return {
    showAdvancedFields: full,
    showCustomModelOption: full,
    showCustomProviderOption: full,
    showDescriptions: full,
    showEffortField: true,
    showProviderPlaceholderOption: full,
    showRequiredIndicators: full,
    showUnavailableEffortOptions: full,
  } as const;
}

export function shouldRevealDependentConfigFields({
  disclosure,
  providerFieldVisible,
  providerValue,
}: {
  disclosure: AgentConfigDisclosure;
  providerFieldVisible: boolean;
  providerValue: string;
}): boolean {
  return (
    disclosure !== "progressive-defaults" ||
    !providerFieldVisible ||
    providerValue.trim().length > 0
  );
}

export function shouldShowModelStatusMessage(
  showDescriptions: boolean,
  status: { message: string; tone: string } | null,
): boolean {
  return showDescriptions || status !== null;
}

export function shouldRenderModelControl({
  discoveredModelOptions,
  modelDiscoveryLoading,
  modelDiscoverySuccessfulEmpty,
  modelIsOptional,
  showCustomModelOption,
}: {
  discoveredModelOptions: readonly { id: string }[] | null;
  modelDiscoveryLoading: boolean;
  modelDiscoverySuccessfulEmpty: boolean;
  modelIsOptional: boolean;
  showCustomModelOption: boolean;
}): boolean {
  if (!modelIsOptional) return true;
  if (modelDiscoveryLoading) return false;
  const hasExplicitModel = (discoveredModelOptions ?? []).some(
    (option) => option.id.trim().length > 0,
  );
  if (hasExplicitModel || showCustomModelOption) return true;
  return !modelDiscoverySuccessfulEmpty;
}
