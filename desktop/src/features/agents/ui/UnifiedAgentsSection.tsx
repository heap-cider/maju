import * as React from "react";
import { useQuery } from "@tanstack/react-query";
import { AlertTriangle, ChevronDown, ChevronRight } from "lucide-react";

import {
  isAgentCardAvatarLoading,
  resolveAgentCardAvatarUrl,
} from "@/features/agents/lib/agentCardAvatar";
import { resolveAgentCardModelLabel } from "@/features/agents/lib/agentCardModelLabel";
import {
  type AgentExecutionLocation,
  findAgentExecutionLocation,
  indexAgentExecutionLocations,
} from "@/features/agents/agentExecutionLocations";
import { friendlyAgentLastError } from "@/features/agents/lib/friendlyAgentLastError";
import { isManagedAgentActive } from "@/features/agents/lib/managedAgentControlActions";
import { useUserProfileQuery } from "@/features/profile/hooks";
import type { AgentPersona, ManagedAgent } from "@/shared/api/types";
import { listLoggedInDevices } from "@/shared/api/tauriDevices";
import type { ProfilePanelOpenOptions } from "@/shared/context/ProfilePanelContext";
import { useFeedbackToasts } from "@/shared/hooks/useToastEffect";
import { useFileImportZone } from "@/shared/hooks/useFileImportZone";
import { Badge } from "@/shared/ui/badge";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/shared/ui/dropdown-menu";
import { IdentityCardSkeleton } from "@/shared/ui/identity-card-skeleton";
import { AgentIdentityCard } from "./AgentIdentityCard";
import { AgentRuntimeAvatarControl } from "./AgentRuntimeAvatarControl";
import { CreateIdentityCard } from "./CreateIdentityCard";
import { PersonaActionsMenu } from "./PersonaActionsMenu";
import { buildUnifiedGroups, pickProfileAgent } from "./unifiedAgentGroups";

type UnifiedAgentsSectionProps = {
  defaultModel: string;
  actionErrorMessage: string | null;
  actionNoticeMessage: string | null;
  agents: ManagedAgent[];
  agentsError: Error | null;
  isActionPending: boolean;
  isAgentsLoading: boolean;
  restartingAgentPubkey: string | null;
  startingAgentPubkey: string | null;
  startingPersonaIds: ReadonlySet<string>;
  onOpenAgentProfile: (
    pubkey: string,
    options?: ProfilePanelOpenOptions,
  ) => void;
  onOpenPersonaProfile: (persona: AgentPersona) => void;
  onRestartAgent: (pubkey: string) => void;
  onStartAgent: (pubkey: string) => void;
  onStartPersona: (persona: AgentPersona) => void;
  personas: AgentPersona[];
  personasError: Error | null;
  personaFeedbackErrorMessage: string | null;
  personaFeedbackNoticeMessage: string | null;
  isPersonasLoading: boolean;
  isPersonasPending: boolean;
  onCreatePersona: () => void;
  onDiscoverPersonas: () => void;
  onDuplicatePersona: (persona: AgentPersona) => void;
  onEditPersona: (persona: AgentPersona) => void;
  onSharePersona: (
    persona: AgentPersona,
    linkedAgent: ManagedAgent | undefined,
    effectiveAvatarUrl: string | null,
  ) => void;
  onDeactivatePersona: (persona: AgentPersona) => void;
  onDeletePersona: (persona: AgentPersona) => void;
  onImportSnapshotFile: (fileBytes: number[], fileName: string) => void;
};

const AGENT_CARD_COLUMN_CLASS = "w-full";
export const AGENT_CARD_GRID_COLUMNS_CLASS =
  "grid-cols-[repeat(auto-fill,minmax(220px,240px))]";
export const IDENTITY_CARD_GRID_CLASS = `${AGENT_CARD_COLUMN_CLASS} ${AGENT_CARD_GRID_COLUMNS_CLASS} grid justify-start gap-3 [@container(max-width:40rem)]:justify-center`;

export function UnifiedAgentsSection(props: UnifiedAgentsSectionProps) {
  const {
    actionErrorMessage,
    actionNoticeMessage,
    defaultModel,
    agents,
    agentsError,
    isActionPending,
    isAgentsLoading,
    restartingAgentPubkey,
    startingAgentPubkey,
    startingPersonaIds,
    onOpenAgentProfile,
    onOpenPersonaProfile,
    onRestartAgent,
    onStartAgent,
    onStartPersona,
    personas,
    personasError,
    personaFeedbackErrorMessage,
    personaFeedbackNoticeMessage,
    isPersonasLoading,
    isPersonasPending,
    onCreatePersona,
    onDiscoverPersonas,
    onDuplicatePersona,
    onEditPersona,
    onSharePersona,
    onDeactivatePersona,
    onDeletePersona,
    onImportSnapshotFile,
  } = props;

  const { groups, ungrouped, unknown } = React.useMemo(
    () => buildUnifiedGroups(personas, agents),
    [personas, agents],
  );
  const devicesQuery = useQuery({
    enabled: agents.length > 0,
    queryKey: ["logged-in-devices"],
    queryFn: listLoggedInDevices,
    refetchInterval: 15_000,
    staleTime: 5_000,
  });
  const executionLocations = React.useMemo(
    () => indexAgentExecutionLocations(devicesQuery.data ?? []),
    [devicesQuery.data],
  );
  const [collapsed, setCollapsed] = React.useState<Set<string>>(new Set());
  const {
    fileInputRef,
    isDragOver,
    dropHandlers,
    handleFileChange,
    openFilePicker,
  } = useFileImportZone({ onImportFile: onImportSnapshotFile });

  function toggle(key: string) {
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  }

  useFeedbackToasts(actionNoticeMessage, actionErrorMessage);
  useFeedbackToasts(personaFeedbackNoticeMessage, personaFeedbackErrorMessage);
  const isLoading = isAgentsLoading || isPersonasLoading;

  return (
    <section
      className="relative space-y-4"
      data-testid="agents-library-personas"
      {...dropHandlers}
    >
      {isDragOver ? (
        <div className="pointer-events-none absolute -inset-1 z-10 flex items-center justify-center rounded-2xl border-2 border-dashed border-primary/50 bg-background/80 backdrop-blur-sm">
          <p className="text-sm font-medium text-primary">
            Drop .agent.json or .agent.png to import
          </p>
        </div>
      ) : null}

      <input
        accept=".agent.json,.agent.png"
        className="hidden"
        onChange={handleFileChange}
        ref={fileInputRef}
        type="file"
      />

      {isLoading ? <LoadingSkeleton /> : null}

      {!isLoading ? (
        <div className="space-y-3" data-testid="unified-agents-groups">
          <div className={IDENTITY_CARD_GRID_CLASS}>
            {groups.map((group) => {
              const profileAgent = pickProfileAgent(group.agents);
              return (
                <AgentPersonaCard
                  actions={(effectiveAvatarUrl, isEffectiveAvatarLoading) => (
                    <PersonaActionsMenu
                      isActionPending={
                        isActionPending || isEffectiveAvatarLoading
                      }
                      isPending={isPersonasPending}
                      persona={group.persona}
                      linkedAgent={profileAgent}
                      onDeactivate={onDeactivatePersona}
                      onDelete={onDeletePersona}
                      onDuplicate={onDuplicatePersona}
                      onEdit={onEditPersona}
                      onShare={(persona, linkedAgent) =>
                        onSharePersona(persona, linkedAgent, effectiveAvatarUrl)
                      }
                    />
                  )}
                  agent={profileAgent}
                  defaultModel={defaultModel}
                  executionLocation={
                    profileAgent
                      ? findAgentExecutionLocation(
                          executionLocations,
                          profileAgent.pubkey,
                        )
                      : undefined
                  }
                  key={group.persona.id}
                  persona={group.persona}
                  restartingAgentPubkey={restartingAgentPubkey}
                  startingAgentPubkey={startingAgentPubkey}
                  startingPersonaIds={startingPersonaIds}
                  onOpenAgentProfile={onOpenAgentProfile}
                  onOpenPersonaProfile={onOpenPersonaProfile}
                  onRestartAgent={onRestartAgent}
                  onStartAgent={onStartAgent}
                  onStartPersona={onStartPersona}
                />
              );
            })}
            <NewAgentCard
              isPending={isPersonasPending}
              onCreate={onCreatePersona}
              onDiscover={onDiscoverPersonas}
              onImport={openFilePicker}
            />
          </div>

          {unknown.length > 0 ? (
            <CollapsibleAgentGroup
              agents={unknown}
              collapsed={collapsed}
              defaultModel={defaultModel}
              executionLocations={executionLocations}
              groupKey="__unknown__"
              label="Unknown agents"
              restartingAgentPubkey={restartingAgentPubkey}
              startingAgentPubkey={startingAgentPubkey}
              onToggle={toggle}
              onOpenAgentProfile={onOpenAgentProfile}
              onRestartAgent={onRestartAgent}
              onStartAgent={onStartAgent}
            />
          ) : null}
          {ungrouped.length > 0 ? (
            <CollapsibleAgentGroup
              agents={ungrouped}
              collapsed={collapsed}
              defaultModel={defaultModel}
              executionLocations={executionLocations}
              groupKey="__ungrouped__"
              label="Custom agents"
              restartingAgentPubkey={restartingAgentPubkey}
              startingAgentPubkey={startingAgentPubkey}
              onToggle={toggle}
              onOpenAgentProfile={onOpenAgentProfile}
              onRestartAgent={onRestartAgent}
              onStartAgent={onStartAgent}
            />
          ) : null}
        </div>
      ) : null}

      {agentsError ? (
        <p
          className={`${AGENT_CARD_COLUMN_CLASS} rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive`}
        >
          {agentsError.message}
        </p>
      ) : null}
      {personasError ? (
        <p
          className={`${AGENT_CARD_COLUMN_CLASS} rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive`}
        >
          {personasError.message}
        </p>
      ) : null}
    </section>
  );
}

function AgentPersonaCard({
  actions,
  agent,
  defaultModel,
  executionLocation,
  persona,
  restartingAgentPubkey,
  startingAgentPubkey,
  startingPersonaIds,
  onOpenAgentProfile,
  onOpenPersonaProfile,
  onRestartAgent,
  onStartAgent,
  onStartPersona,
}: {
  actions?: (
    effectiveAvatarUrl: string | null,
    isEffectiveAvatarLoading: boolean,
  ) => React.ReactNode;
  agent: ManagedAgent | undefined;
  defaultModel: string;
  executionLocation: AgentExecutionLocation | undefined;
  persona: AgentPersona;
  restartingAgentPubkey: string | null;
  startingAgentPubkey: string | null;
  startingPersonaIds: ReadonlySet<string>;
  onOpenAgentProfile: (
    pubkey: string,
    options?: ProfilePanelOpenOptions,
  ) => void;
  onOpenPersonaProfile: (persona: AgentPersona) => void;
  onRestartAgent: (pubkey: string) => void;
  onStartAgent: (pubkey: string) => void;
  onStartPersona: (persona: AgentPersona) => void;
}) {
  const title = persona.displayName;
  const modelLabel = resolveAgentCardModelLabel({
    agent,
    personaModel: persona.model,
    defaultModel,
  });
  const isLocallyActive = agent ? isManagedAgentActive(agent) : false;
  const isActive = Boolean(
    isLocallyActive || executionLocation?.representativeDeviceName,
  );
  const profileQuery = useUserProfileQuery(agent?.pubkey);
  const avatarUrl = agent
    ? resolveAgentCardAvatarUrl(profileQuery.data?.avatarUrl, persona.avatarUrl)
    : persona.avatarUrl;
  const friendlyError = agent
    ? friendlyAgentLastError(agent.lastError, agent.lastErrorCode)?.copy
    : null;
  const visibleError =
    executionLocation?.representativeDeviceName ||
    executionLocation?.currentDeviceIsStandby
      ? null
      : friendlyError;
  const opensRuntimeTab = Boolean(agent && visibleError && !isActive);

  return (
    <AgentIdentityCard
      actions={actions?.(
        avatarUrl,
        isAgentCardAvatarLoading(Boolean(agent), profileQuery.isPending),
      )}
      ariaLabel={`${title} agent profile`}
      avatar={
        agent ? (
          <AgentRuntimeAvatarControl
            activeTestId={`agent-runtime-active-${agent.pubkey}`}
            activeLabel={executionAriaLabel(title, executionLocation)}
            avatarUrl={avatarUrl}
            errorLabel={visibleError}
            errorTestId={`agent-runtime-error-${agent.pubkey}`}
            isActive={isActive}
            isRestarting={restartingAgentPubkey === agent.pubkey}
            isStarting={startingAgentPubkey === agent.pubkey}
            label={title}
            requiresRestart={agent.needsRestart}
            startTestId={`agent-runtime-start-${agent.pubkey}`}
            onOpenError={() => {
              onOpenAgentProfile(agent.pubkey, { tab: "runtime" });
            }}
            onStart={() =>
              agent.needsRestart
                ? onRestartAgent(agent.pubkey)
                : onStartAgent(agent.pubkey)
            }
          />
        ) : (
          <AgentRuntimeAvatarControl
            activeTestId={`persona-runtime-active-${persona.id}`}
            avatarUrl={avatarUrl}
            isActive={false}
            isStarting={startingPersonaIds.has(persona.id)}
            label={title}
            startTestId={`persona-runtime-start-${persona.id}`}
            onStart={() => onStartPersona(persona)}
          />
        )
      }
      avatarUrl={avatarUrl}
      dataTestId={`persona-agent-row-${persona.id}`}
      label={title}
      modelLabel={modelLabel}
      onClick={() => {
        if (agent) {
          onOpenAgentProfile(
            agent.pubkey,
            opensRuntimeTab ? { tab: "runtime" } : undefined,
          );
          return;
        }
        onOpenPersonaProfile(persona);
      }}
      statusBadge={
        agent?.personaOrphaned ? (
          <Badge className="gap-1" variant="warning">
            <AlertTriangle className="h-3 w-3" />
            Configuration missing
          </Badge>
        ) : agent ? (
          <AgentCardStatus
            agent={agent}
            executionLocation={executionLocation}
            isLocallyActive={isLocallyActive}
          />
        ) : null
      }
    />
  );
}

function StandaloneAgentCard({
  agent,
  defaultModel,
  executionLocation,
  restartingAgentPubkey,
  startingAgentPubkey,
  onOpenAgentProfile,
  onRestartAgent,
  onStartAgent,
}: {
  agent: ManagedAgent;
  defaultModel: string;
  executionLocation: AgentExecutionLocation | undefined;
  restartingAgentPubkey: string | null;
  startingAgentPubkey: string | null;
  onOpenAgentProfile: (
    pubkey: string,
    options?: ProfilePanelOpenOptions,
  ) => void;
  onRestartAgent: (pubkey: string) => void;
  onStartAgent: (pubkey: string) => void;
}) {
  const title = agent.name;
  const profileQuery = useUserProfileQuery(agent.pubkey);
  const friendlyError = friendlyAgentLastError(
    agent.lastError,
    agent.lastErrorCode,
  )?.copy;
  const isLocallyActive = isManagedAgentActive(agent);
  const isActive = Boolean(
    isLocallyActive || executionLocation?.representativeDeviceName,
  );
  const visibleError =
    executionLocation?.representativeDeviceName ||
    executionLocation?.currentDeviceIsStandby
      ? null
      : friendlyError;
  const opensRuntimeTab = Boolean(visibleError && !isActive);

  return (
    <AgentIdentityCard
      ariaLabel={`${title} agent profile`}
      avatar={
        <AgentRuntimeAvatarControl
          activeTestId={`agent-runtime-active-${agent.pubkey}`}
          activeLabel={executionAriaLabel(title, executionLocation)}
          avatarUrl={profileQuery.data?.avatarUrl}
          errorLabel={visibleError}
          errorTestId={`agent-runtime-error-${agent.pubkey}`}
          isActive={isActive}
          isRestarting={restartingAgentPubkey === agent.pubkey}
          isStarting={startingAgentPubkey === agent.pubkey}
          label={title}
          requiresRestart={agent.needsRestart}
          startTestId={`agent-runtime-start-${agent.pubkey}`}
          onOpenError={() => {
            onOpenAgentProfile(agent.pubkey, { tab: "runtime" });
          }}
          onStart={() =>
            agent.needsRestart
              ? onRestartAgent(agent.pubkey)
              : onStartAgent(agent.pubkey)
          }
        />
      }
      avatarUrl={profileQuery.data?.avatarUrl}
      dataTestId={`managed-agent-${agent.pubkey}`}
      label={title}
      modelLabel={resolveAgentCardModelLabel({
        agent,
        personaModel: null,
        defaultModel,
      })}
      onClick={() => {
        onOpenAgentProfile(
          agent.pubkey,
          opensRuntimeTab ? { tab: "runtime" } : undefined,
        );
      }}
      statusBadge={
        agent.personaOrphaned ? (
          <Badge className="gap-1" variant="warning">
            <AlertTriangle className="h-3 w-3" />
            Configuration missing
          </Badge>
        ) : (
          <AgentCardStatus
            agent={agent}
            executionLocation={executionLocation}
            isLocallyActive={isLocallyActive}
          />
        )
      }
    />
  );
}

function executionAriaLabel(
  agentName: string,
  executionLocation: AgentExecutionLocation | undefined,
): string {
  if (executionLocation?.representativeIsCurrent) {
    return `${agentName}, 이 기기에서 실행 중`;
  }
  if (executionLocation?.representativeDeviceName) {
    return `${agentName}, ${executionLocation.representativeDeviceName}에서 실행 중`;
  }
  return `${agentName} 실행 중`;
}

function AgentCardStatus({
  agent,
  executionLocation,
  isLocallyActive,
}: {
  agent: ManagedAgent;
  executionLocation: AgentExecutionLocation | undefined;
  isLocallyActive: boolean;
}) {
  const representativeLabel = executionLocation?.representativeIsCurrent
    ? "이 기기에서 실행 중"
    : executionLocation?.representativeDeviceName
      ? `${executionLocation.representativeDeviceName}에서 실행 중`
      : isLocallyActive
        ? "이 기기에서 실행 중"
        : null;
  const isCurrentStandby = Boolean(
    executionLocation?.currentDeviceIsStandby &&
      !executionLocation.representativeIsCurrent,
  );
  const hasWarning = agent.personaOrphaned;

  if (!hasWarning && !representativeLabel && !isCurrentStandby) {
    return null;
  }

  return (
    <div className="mt-1 flex min-w-0 flex-col items-start gap-1">
      {agent.personaOrphaned ? (
        <Badge className="max-w-full gap-1" variant="warning">
          <AlertTriangle className="h-3 w-3 shrink-0" />
          <span className="truncate">Configuration missing</span>
        </Badge>
      ) : null}
      {representativeLabel ? (
        <span
          className="flex min-w-0 max-w-full items-center gap-1.5 text-xs font-medium text-emerald-600 dark:text-emerald-400"
          data-testid={`agent-execution-location-${agent.pubkey}`}
          title={representativeLabel}
        >
          <span className="h-1.5 w-1.5 shrink-0 rounded-full bg-current" />
          <span className="truncate">{representativeLabel}</span>
        </span>
      ) : null}
      {isCurrentStandby ? (
        <span
          className="max-w-full truncate text-2xs font-medium text-muted-foreground"
          data-testid={`agent-standby-location-${agent.pubkey}`}
          title="이 기기는 자동 대기 중"
        >
          이 기기는 자동 대기 중
        </span>
      ) : null}
    </div>
  );
}

function NewAgentCard({
  isPending,
  onCreate,
  onDiscover,
  onImport,
}: {
  isPending: boolean;
  onCreate: () => void;
  onDiscover: () => void;
  onImport: () => void;
}) {
  return (
    <DropdownMenu modal={false}>
      <DropdownMenuTrigger asChild>
        <CreateIdentityCard ariaLabel="New agent" dataTestId="new-agent-card" />
      </DropdownMenuTrigger>
      <DropdownMenuContent
        align="start"
        onCloseAutoFocus={(event) => event.preventDefault()}
      >
        <DropdownMenuItem disabled={isPending} onClick={onCreate}>
          Create agent
        </DropdownMenuItem>
        <DropdownMenuItem disabled={isPending} onClick={onDiscover}>
          Discover agents
        </DropdownMenuItem>
        <DropdownMenuItem
          data-testid="import-agent-snapshot-menu-item"
          disabled={isPending}
          onClick={onImport}
        >
          Import
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

function LoadingSkeleton() {
  return (
    <div className={IDENTITY_CARD_GRID_CLASS}>
      <IdentityCardSkeleton
        footerSubtitleWidthClass="w-14"
        footerTitleWidthClass="w-24"
      />
      <IdentityCardSkeleton
        footerSubtitleWidthClass="w-20"
        footerTitleWidthClass="w-32"
      />
      <IdentityCardSkeleton
        footerSubtitleWidthClass="w-16"
        footerTitleWidthClass="w-28"
      />
    </div>
  );
}

function CollapsibleAgentGroup({
  groupKey,
  label,
  agents,
  collapsed,
  defaultModel,
  executionLocations,
  restartingAgentPubkey,
  startingAgentPubkey,
  onToggle,
  onOpenAgentProfile,
  onRestartAgent,
  onStartAgent,
}: {
  groupKey: string;
  label: string;
  agents: ManagedAgent[];
  collapsed: ReadonlySet<string>;
  defaultModel: string;
  executionLocations: ReadonlyMap<string, AgentExecutionLocation>;
  restartingAgentPubkey: string | null;
  startingAgentPubkey: string | null;
  onToggle: (key: string) => void;
  onOpenAgentProfile: (
    pubkey: string,
    options?: ProfilePanelOpenOptions,
  ) => void;
  onRestartAgent: (pubkey: string) => void;
  onStartAgent: (pubkey: string) => void;
}) {
  const isCollapsed = collapsed.has(groupKey);
  return (
    <div className={`${AGENT_CARD_COLUMN_CLASS} space-y-2`}>
      <button
        className="group flex items-center gap-2 rounded-md px-1 py-1 text-left transition-colors hover:bg-muted/50"
        onClick={() => onToggle(groupKey)}
        type="button"
      >
        {isCollapsed ? (
          <ChevronRight className="h-4 w-4 shrink-0 text-muted-foreground transition-transform group-hover:translate-x-0.5" />
        ) : (
          <ChevronDown className="h-4 w-4 shrink-0 text-muted-foreground" />
        )}
        <span className="text-sm font-medium">{label}</span>
        <span className="text-xs text-muted-foreground">({agents.length})</span>
      </button>
      {!isCollapsed ? (
        <div className={IDENTITY_CARD_GRID_CLASS}>
          {agents.map((agent) => (
            <StandaloneAgentCard
              agent={agent}
              defaultModel={defaultModel}
              executionLocation={findAgentExecutionLocation(
                executionLocations,
                agent.pubkey,
              )}
              key={agent.pubkey}
              restartingAgentPubkey={restartingAgentPubkey}
              startingAgentPubkey={startingAgentPubkey}
              onOpenAgentProfile={onOpenAgentProfile}
              onRestartAgent={onRestartAgent}
              onStartAgent={onStartAgent}
            />
          ))}
        </div>
      ) : null}
    </div>
  );
}
