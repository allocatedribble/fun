import type {
  CommandbarContextSnapshot,
  CommandbarIntent,
  CommandbarIntentKind,
  CommandbarMode,
  EditorUiState,
  ExperienceScope,
  ToolCallRisk
} from '../types';

export function normalizeCommandbarQuery(input: string): string {
  return input.replace(/^[>?#/@]\s*/, '').trim();
}

export function looksLikePath(input: string): boolean {
  return /^[a-z]:[\\/]/i.test(input) || input.startsWith('\\\\') || input.startsWith('./') || input.startsWith('../');
}

export function isProjectSwitchQuery(query: string): boolean {
  return /(^|\s)(project|projects|workspace|workspaces|recent project|switch project|open project)(\s|$)/i.test(query);
}

export function projectQueryTail(query: string): string {
  return query
    .replace(/\b(recent|switch|open|select|choose|show|list|to|project|projects|workspace|workspaces)\b/gi, ' ')
    .replace(/\s+/g, ' ')
    .trim();
}

export function inferCommandbarMode(input: string): CommandbarMode {
  if (!input) {
    return 'identity';
  }
  if (input.startsWith('>')) {
    return 'command';
  }
  if (input.startsWith('?')) {
    return 'ask';
  }
  if (input.startsWith('/')) {
    return 'tool';
  }
  if (input.startsWith('@')) {
    return 'remote';
  }
  if (isProjectSwitchQuery(input) || looksLikePath(input)) {
    return 'project';
  }
  if (/\b(why|explain|reason|how|llm|ask)\b/i.test(input)) {
    return 'ask';
  }
  if (/\b(server|client|connected|remote)\b/i.test(input)) {
    return 'remote';
  }
  return 'search';
}

export function commandbarContextSnapshot(state: EditorUiState): CommandbarContextSnapshot {
  return {
    projectId: state.project?.id,
    projectName: state.project?.display_name,
    activeSceneId: state.activeSceneId,
    selectedEntityId: state.entities.selectedId,
    activeEditorContextId: state.activeEditorContextId,
    clientId: state.liveClientStatus?.id,
    serverId: state.serverRuntime?.id,
    diagnosticCount: state.diagnostics.length + (state.runtimeDiagnostics?.records.length ?? 0),
    commandCount: state.commands.length
  };
}

export function routeCommandbarIntent(
  input: string,
  state: EditorUiState,
  activeScope: ExperienceScope
): CommandbarIntent {
  const query = normalizeCommandbarQuery(input);
  const lowerQuery = query.toLowerCase();
  const preferredToolIds = new Set<string>();
  const scopes: ExperienceScope[] = [activeScope];
  let intent: CommandbarIntentKind = 'search';
  let confidence = query ? 0.56 : 0;
  let needsLlm = false;
  let riskCeiling: ToolCallRisk = 'read';

  if (input.startsWith('>')) {
    intent = 'run';
    confidence = 0.92;
    preferredToolIds.add('host.commands.list');
  } else if (input.startsWith('/')) {
    intent = 'tool';
    confidence = 0.92;
    preferredToolIds.add('editor.tools.list');
  } else if (input.startsWith('?')) {
    intent = 'ask';
    confidence = 0.9;
    needsLlm = true;
    preferredToolIds.add('llm.reason_over_scope');
  } else if (input.startsWith('@')) {
    intent = 'inspect';
    confidence = 0.78;
    scopes.push({ type: 'all_connected' });
    preferredToolIds.add('entity.search');
  } else if (input.startsWith('#')) {
    intent = 'diagnose';
    confidence = 0.82;
    preferredToolIds.add('runtime.diagnostics.list');
  } else if (isProjectSwitchQuery(query) || looksLikePath(query)) {
    intent = 'open';
    confidence = 0.84;
    preferredToolIds.add('projects.recent.list');
    preferredToolIds.add('project.current.get');
  } else if (/\b(why|explain|black|broken|failed|warning|error|diagnostic)\b/.test(lowerQuery)) {
    intent = 'diagnose';
    confidence = 0.76;
    needsLlm = /\b(why|explain|reason|how)\b/.test(lowerQuery);
    preferredToolIds.add('runtime.diagnostics.list');
    preferredToolIds.add('preview.renderer.status.get');
  } else if (/\b(open|switch|go to|show|focus|frame)\b/.test(lowerQuery)) {
    intent = /\b(frame|focus|select)\b/.test(lowerQuery) ? 'navigate' : 'open';
    confidence = 0.72;
    preferredToolIds.add('context.set.preview');
  } else if (/\b(play|launch|reload|run|start|stop|build)\b/.test(lowerQuery)) {
    intent = 'run';
    confidence = 0.74;
    preferredToolIds.add('preview.renderer.status.get');
  } else if (/\b(move|rename|set|change|save|connect|create|add|delete|remove|clear|reset)\b/.test(lowerQuery)) {
    intent = 'mutate';
    confidence = 0.68;
    riskCeiling = /\b(delete|remove|clear|reset|destroy)\b/.test(lowerQuery) ? 'destructive' : 'write';
    preferredToolIds.add('entity.transform.patch');
  } else if (/\b(host|publish|deploy|sync|backend|server browser|cdn|matchmaking)\b/.test(lowerQuery)) {
    intent = 'backend';
    confidence = 0.74;
    riskCeiling = 'network';
    preferredToolIds.add('backend.experience.host');
  }

  if (/\b(server|client|connected|runtime)\b/.test(lowerQuery)) {
    scopes.push({ type: 'all_connected' });
  }

  const selected = state.entities.selectedDetails;
  const entities = selected
    ? [{ kind: 'entity', name: selected.row.name, confidence: 0.88 }]
    : [...Object.values(state.entities.rowsByIndex)]
        .filter((row) => lowerQuery.includes(row.name.toLowerCase()))
        .slice(0, 3)
        .map((row) => ({ kind: 'entity', name: row.name, confidence: 0.72 }));

  return {
    query,
    intent,
    confidence,
    scopes: dedupeScopes(scopes),
    entities,
    preferredToolIds: [...preferredToolIds],
    needsLlm,
    riskCeiling
  };
}

export function intentSummary(intent: CommandbarIntent): string | undefined {
  if (!intent.query) {
    return undefined;
  }
  const llm = intent.needsLlm ? ' + scoped LLM after retrieval' : '';
  const tools = intent.preferredToolIds.length > 0 ? ` - ${intent.preferredToolIds.slice(0, 3).join(', ')}` : '';
  return `${intent.intent} intent ${Math.round(intent.confidence * 100)}%${llm}${tools}`;
}

function dedupeScopes(scopes: ExperienceScope[]): ExperienceScope[] {
  const seen = new Set<string>();
  return scopes.filter((scope) => {
    const key = JSON.stringify(scope);
    if (seen.has(key)) {
      return false;
    }
    seen.add(key);
    return true;
  });
}
