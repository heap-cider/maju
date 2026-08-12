import type { AgentPersona, AgentTeam } from "@/shared/api/types";

function getAvailablePersonaIds(personas: AgentPersona[]): Set<string> {
  return new Set(personas.map((persona) => persona.id));
}

export function copySelectedPersonaIds(personaIds: string[]): string[] {
  return [...personaIds];
}

export function countMissingPersonaIds(
  personaIds: string[],
  personas: AgentPersona[],
): number {
  const availablePersonaIds = getAvailablePersonaIds(personas);
  return personaIds.filter((personaId) => !availablePersonaIds.has(personaId))
    .length;
}

export function filterAvailablePersonaIds(
  personaIds: string[],
  personas: AgentPersona[],
): string[] {
  const availablePersonaIds = getAvailablePersonaIds(personas);
  return personaIds.filter((personaId) => availablePersonaIds.has(personaId));
}

export function orderPersonasByInitiallySelected(
  personas: AgentPersona[],
  initialSelectedPersonaIds: string[],
): AgentPersona[] {
  const selectedIds = new Set(initialSelectedPersonaIds);
  const selected: AgentPersona[] = [];
  const unselected: AgentPersona[] = [];

  for (const persona of personas) {
    if (selectedIds.has(persona.id)) {
      selected.push(persona);
    } else {
      unselected.push(persona);
    }
  }

  return [...selected, ...unselected];
}

export function indexOtherTeamAssignments(
  teams: readonly AgentTeam[],
  currentTeamId?: string,
): ReadonlyMap<string, AgentTeam> {
  const assignments = new Map<string, AgentTeam>();
  for (const team of teams) {
    if (team.id === currentTeamId) continue;
    for (const personaId of team.personaIds) {
      assignments.set(personaId, team);
    }
  }
  return assignments;
}
