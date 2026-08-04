export type AgentModelInfo = {
  id: string;
  name: string | null;
  description: string | null;
};

export type AcpConfigScalar = string | number | boolean;

export type AcpConfigOptionValue = {
  value: string;
  displayName: string | null;
};

export type AcpConfigOptionEntry = {
  configId: string;
  category: string | null;
  displayName: string | null;
  description: string | null;
  optionType: string | null;
  currentValue: AcpConfigScalar | null;
  options: AcpConfigOptionValue[];
};

export type AgentModelsResponse = {
  agentName: string;
  agentVersion: string;
  models: AgentModelInfo[];
  agentDefaultModel: string | null;
  selectedModel: string | null;
  supportsSwitching: boolean;
  configOptions?: AcpConfigOptionEntry[];
};
