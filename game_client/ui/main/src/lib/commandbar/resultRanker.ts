import type { CommandbarIntent, CommandbarResult, ToolCallRisk } from '../types';

export type CommandbarResultGroup =
  | 'Best'
  | 'Commands'
  | 'Project'
  | 'Entities'
  | 'Runtime'
  | 'Diagnostics'
  | 'Tools'
  | 'Backend'
  | 'LLM';

const groupOrder: Record<CommandbarResultGroup, number> = {
  Best: 0,
  Commands: 1,
  Project: 2,
  Entities: 3,
  Runtime: 4,
  Diagnostics: 5,
  Tools: 6,
  Backend: 7,
  LLM: 8
};

export function matchesQuery(query: string, ...parts: Array<string | undefined>): boolean {
  if (!query) {
    return true;
  }
  const haystack = parts.filter(Boolean).join(' ').toLowerCase();
  const tokens = query.toLowerCase().split(/\s+/).filter(Boolean);
  return tokens.every((token) => haystack.includes(token));
}

export function scoreQuery(query: string, ...parts: Array<string | undefined>): number {
  if (!query) {
    return 1;
  }
  const haystack = parts.filter(Boolean).join(' ').toLowerCase();
  const normalized = query.toLowerCase();
  if (haystack.startsWith(normalized)) {
    return 100;
  }
  if (haystack.includes(normalized)) {
    return 72;
  }
  const tokens = normalized.split(/\s+/).filter(Boolean);
  return tokens.reduce((score, token) => score + (haystack.includes(token) ? 12 : 0), 0);
}

export function commandbarResultGroup(result: CommandbarResult): CommandbarResultGroup {
  if (result.type === 'llm_answer') {
    return 'LLM';
  }
  if (result.type === 'diagnostic') {
    return 'Diagnostics';
  }
  if (result.type === 'client' || result.type === 'server') {
    return 'Runtime';
  }
  if (result.type === 'entity') {
    return 'Entities';
  }
  if (result.type === 'project' || result.type === 'file' || result.type === 'asset') {
    return 'Project';
  }
  if (result.type === 'tool') {
    return result.metadata?.category === 'backend' ? 'Backend' : 'Tools';
  }
  return 'Commands';
}

export function resultRisk(result: CommandbarResult): ToolCallRisk | undefined {
  if (result.action?.kind === 'tool_call_preview') {
    return result.action.toolCall.risk;
  }
  const risk = result.metadata?.risk;
  return typeof risk === 'string' ? (risk as ToolCallRisk) : undefined;
}

export function rankCommandbarResults(
  results: CommandbarResult[],
  query: string,
  intent: CommandbarIntent,
  cap: number
): CommandbarResult[] {
  return results
    .map((result, index) => ({
      result,
      rank:
        result.score +
        intentBonus(result, intent) -
        riskPenalty(result) -
        staleScopePenalty(result, intent) -
        index * 0.001
    }))
    .sort((left, right) => {
      if (right.rank !== left.rank) {
        return right.rank - left.rank;
      }
      return (
        groupOrder[commandbarResultGroup(left.result)] - groupOrder[commandbarResultGroup(right.result)] ||
        left.result.title.localeCompare(right.result.title)
      );
    })
    .map(({ result }) => result)
    .slice(0, cap);
}

export function groupedCommandbarResults(results: CommandbarResult[]): Array<{ group: CommandbarResultGroup; results: CommandbarResult[] }> {
  const [best, ...rest] = results;
  if (!best) {
    return [];
  }
  const groups = new Map<CommandbarResultGroup, CommandbarResult[]>();
  for (const result of rest) {
    const group = commandbarResultGroup(result);
    groups.set(group, [...(groups.get(group) ?? []), result]);
  }
  return [
    { group: 'Best' as const, results: [best] },
    ...[...groups.entries()]
    .sort(([left], [right]) => groupOrder[left] - groupOrder[right])
    .map(([group, groupResults]) => ({ group, results: groupResults }))
  ];
}

function intentBonus(result: CommandbarResult, intent: CommandbarIntent): number {
  if (intent.preferredToolIds.some((toolId) => result.id.includes(toolId))) {
    return 24;
  }
  const group = commandbarResultGroup(result);
  if (intent.intent === 'diagnose' && group === 'Diagnostics') return 18;
  if (intent.intent === 'open' && group === 'Project') return 18;
  if (intent.intent === 'inspect' && group === 'Entities') return 16;
  if (intent.intent === 'run' && (group === 'Commands' || group === 'Runtime')) return 14;
  if (intent.intent === 'backend' && group === 'Backend') return 22;
  if (intent.intent === 'ask' && group === 'LLM') return 20;
  if (intent.intent === 'tool' && group === 'Tools') return 18;
  return 0;
}

function riskPenalty(result: CommandbarResult): number {
  const risk = resultRisk(result);
  if (risk === 'destructive') return 28;
  if (risk === 'admin') return 24;
  if (risk === 'network') return 18;
  if (risk === 'write') return 12;
  return 0;
}

function staleScopePenalty(result: CommandbarResult, intent: CommandbarIntent): number {
  if (intent.scopes.some((scope) => JSON.stringify(scope) === JSON.stringify(result.scope))) {
    return 0;
  }
  return result.scope.type === 'all_connected' ? 8 : 0;
}
