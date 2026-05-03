import type {
  CommandbarIntent,
  CommandbarResult,
  CommandbarAction,
  Diagnostic,
  DiagnosticSeverity,
  EditorUiState,
  ExperienceScope,
  SearchIndexRecord
} from '../types';
import {
  inferCommandbarMode,
  isProjectSwitchQuery,
  looksLikePath,
  normalizeCommandbarQuery,
  projectQueryTail,
  routeCommandbarIntent
} from './intentRouter';
import { matchesQuery, rankCommandbarResults, scoreQuery } from './resultRanker';
import { scopeId } from './scopeRegistry';
import { commandbarResultFromTool, editorToolDefinitions, toolCallFromDefinition } from './toolRegistry';

export function buildCommandbarResults(
  state: EditorUiState,
  input: string,
  activeScope: ExperienceScope,
  cap: number,
  routedIntent: CommandbarIntent = routeCommandbarIntent(input, state, activeScope)
): CommandbarResult[] {
  const query = normalizeCommandbarQuery(input);
  const lowerQuery = query.toLowerCase();
  const projectIntent = isProjectSwitchQuery(query);
  const projectQuery = projectIntent ? projectQueryTail(query) : query;
  const results: CommandbarResult[] = [];
  const push = (result: CommandbarResult, keywords: string[] = []): void => {
    if (matchesQuery(query, result.title, result.subtitle, result.preview, result.type, ...keywords)) {
      results.push(result);
    }
  };
  const pushProjectIntent = (result: CommandbarResult, keywords: string[] = []): void => {
    if (!projectIntent) {
      return;
    }
    if (!projectQuery || matchesQuery(projectQuery, result.title, result.subtitle, result.preview, ...keywords)) {
      results.push(result);
    }
  };

  for (const tool of editorToolDefinitions(state)) {
    const score = scoreQuery(query, tool.title, tool.description, tool.id, tool.category, ...tool.keywords);
    const preferred = routedIntent.preferredToolIds.includes(tool.id);
    if (score > 0 || preferred) {
      push(commandbarResultFromTool(tool, activeScope, query, score + (preferred ? 30 : 0)), tool.keywords);
    }
  }

  for (const result of buildProjectSwitchResults(state, activeScope, projectQuery)) {
    pushProjectIntent(result, ['project', 'workspace', 'switch', 'recent', 'open', 'browse']);
  }

  for (const context of state.availableContexts) {
    push({
      id: `context:${context.id}`,
      type: 'command',
      title: `Switch to ${context.label}`,
      subtitle: context.status,
      score: scoreQuery(query, context.label, context.status, 'context tab mode'),
      scope: activeScope,
      action: { kind: 'set_context', contextId: context.id }
    });
  }

  const projectResults = [
    ...(state.project
      ? [{ id: state.project.id, display_name: state.project.display_name, root_path: state.project.root_path }]
      : []),
    ...state.recentProjects,
    ...state.projects
  ];
  const seenProjects = new Set<string>();
  for (const project of projectResults) {
    const key = `${project.id}:${project.root_path}`;
    if (seenProjects.has(key)) {
      continue;
    }
    seenProjects.add(key);
    push({
      id: `project:${project.id}`,
      type: 'project',
      title: project.display_name,
      subtitle: project.root_path,
      score: scoreQuery(query, project.display_name, project.root_path, 'project workspace'),
      scope: { type: 'current_project', projectId: project.id },
      action: { kind: 'open_project', path: project.root_path }
    });
  }

  if (looksLikePath(query)) {
    push({
      id: `project-path:${query}`,
      type: 'project',
      title: 'Open Project Path',
      subtitle: query,
      score: 100,
      scope: activeScope,
      action: { kind: 'open_project', path: query }
    });
  }

  for (const scene of state.project?.bsn_index.records ?? []) {
    push({
      id: `scene:${scene.id}`,
      type: 'entity',
      title: scene.scene_function_name ?? scene.detected_names[0] ?? scene.invocation_kind,
      subtitle: `${scene.domain ?? 'shared'} scene - ${scene.file_path}`,
      score: scoreQuery(
        query,
        scene.scene_function_name ?? '',
        scene.file_path,
        scene.invocation_kind,
        ...scene.detected_names,
        ...scene.component_type_tokens
      ),
      scope: state.project ? { type: 'current_project', projectId: state.project.id } : activeScope,
      preview: scene.component_type_tokens.slice(0, 8).join(', ')
    });
  }

  for (const entity of Object.values(state.entities.rowsByIndex).slice(0, 240)) {
    push({
      id: `entity:${entity.id}`,
      type: 'entity',
      title: entity.name,
      subtitle: `${entity.domain} - ${entity.source}`,
      score: scoreQuery(query, entity.name, entity.domain, entity.source, entity.source_kind),
      scope:
        entity.domain === 'client'
          ? state.liveClientStatus
            ? { type: 'client', clientId: state.liveClientStatus.id }
            : activeScope
          : entity.domain === 'server' && state.serverRuntime
            ? { type: 'server', serverId: state.serverRuntime.id }
            : activeScope,
      action: { kind: 'select_entity', entityId: entity.id },
      metadata: { componentCount: entity.component_count, revision: entity.revision }
    });
  }

  state.diagnostics.forEach((diagnostic, index) => {
    const severity = diagnosticSeverity(diagnostic.level);
    const tags = diagnosticTags(diagnostic);
    push({
      id: `diagnostic:${diagnostic.code}:${index}`,
      type: 'diagnostic',
      title: diagnostic.message,
      subtitle: `${severity} - ${diagnostic.code}`,
      score: scoreQuery(query, diagnostic.message, diagnostic.code, diagnostic.target_path ?? '', ...tags),
      scope: activeScope,
      action: { kind: 'filter_diagnostics', severity },
      metadata: { severity, tags }
    });
  });

  for (const record of state.runtimeDiagnostics?.records.slice(-80) ?? []) {
    push({
      id: `runtime-diagnostic:${record.sequence}`,
      type: 'diagnostic',
      title: record.message,
      subtitle: `${record.target} ${record.kind} #${record.sequence}`,
      score: scoreQuery(query, record.message, record.target, record.kind, record.source ?? ''),
      scope:
        record.target === 'client' && state.liveClientStatus
          ? { type: 'client', clientId: state.liveClientStatus.id }
          : record.target === 'server' && state.serverRuntime
            ? { type: 'server', serverId: state.serverRuntime.id }
            : activeScope,
      action: { kind: 'filter_diagnostics', tag: record.target },
      metadata: { severity: diagnosticSeverity(record.level), tags: [record.target, record.kind] }
    });
  }

  if (state.liveClientStatus) {
    push({
      id: `client:${state.liveClientStatus.id}`,
      type: 'client',
      title: 'Current Fun Client',
      subtitle: `${state.liveClientStatus.status} - in host`,
      score: scoreQuery(query, 'client current game world', state.liveClientStatus.id),
      scope: { type: 'client', clientId: state.liveClientStatus.id },
      preview: state.liveClientStatus.launch_command.join(' ')
    });
  }

  if (state.serverRuntime) {
    push({
      id: `server:${state.serverRuntime.id}`,
      type: 'server',
      title: 'Local Fun Server',
      subtitle: `${state.serverRuntime.process_status} - ${state.serverRuntime.inspector_status}`,
      score: scoreQuery(query, 'server local authority runtime', state.serverRuntime.id),
      scope: { type: 'server', serverId: state.serverRuntime.id },
      preview: state.serverRuntime.launch_command.join(' ')
    });
  }

  if (routedIntent.needsLlm || inferCommandbarMode(input) === 'ask' || /\b(why|explain|llm|material|graph|diagnostic)\b/.test(lowerQuery)) {
    const definition = editorToolDefinitions(state).find((tool) => tool.id === 'llm.reason_over_scope');
    if (definition) {
      const call = toolCallFromDefinition(definition, activeScope, { query, intent: routedIntent.intent });
      results.push({
        id: 'llm:reason',
        type: 'llm_answer',
        title: 'Ask LLM over current scope',
        subtitle: 'Routes through the model router after fast local retrieval',
        score: 88,
        scope: activeScope,
        action: { kind: 'tool_call_preview', toolCall: call },
        preview: 'Read-only reasoning proposal; cloud routing stays disabled unless project privacy allows it.'
      });
    }
  }

  return rankCommandbarResults(results, query, routedIntent, cap);
}

export function buildSearchIndexRecords(state: EditorUiState, activeScope: ExperienceScope): SearchIndexRecord[] {
  const now = Date.now();
  const records: SearchIndexRecord[] = [];
  const scope = scopeId(activeScope);
  for (const tool of editorToolDefinitions(state)) {
    records.push({
      id: `tool:${tool.id}`,
      scopeId: scope,
      entityType: 'tool',
      title: tool.title,
      subtitle: tool.description,
      tags: [tool.category, tool.risk],
      keywords: [tool.id, ...tool.keywords],
      lastUpdated: now,
      inspectAction: {
        kind: tool.commandId ? 'command' : 'tool_call_preview',
        ...(tool.commandId
          ? { commandId: tool.commandId }
          : { toolCall: toolCallFromDefinition(tool, activeScope, { query: tool.id }) })
      } as CommandbarAction
    });
  }
  for (const scene of state.project?.bsn_index.records ?? []) {
    records.push({
      id: `scene:${scene.id}`,
      scopeId: state.project ? `project:${state.project.id}` : scope,
      entityType: 'scene',
      title: scene.scene_function_name ?? scene.invocation_kind,
      subtitle: scene.file_path,
      path: scene.file_path,
      tags: [scene.domain ?? 'shared', 'scene', 'bsn'],
      keywords: [...scene.detected_names, ...scene.component_type_tokens],
      references: scene.detected_names,
      lastUpdated: now
    });
  }
  for (const entity of Object.values(state.entities.rowsByIndex)) {
    records.push({
      id: `entity:${entity.id}`,
      scopeId: scope,
      entityType: 'entity',
      title: entity.name,
      subtitle: `${entity.domain} ${entity.source_kind}`,
      tags: [entity.domain, entity.source_kind],
      keywords: [entity.id, entity.source],
      lastUpdated: now,
      openAction: { kind: 'select_entity', entityId: entity.id }
    });
  }
  return records;
}

function buildProjectSwitchResults(
  state: EditorUiState,
  activeScope: ExperienceScope,
  projectQuery: string
): CommandbarResult[] {
  const results: CommandbarResult[] = [
    {
      id: 'project-action:browse',
      type: 'project',
      title: 'Browse for Project',
      subtitle: 'Choose a project with the Windows folder picker',
      score: projectQuery ? 90 : 120,
      scope: activeScope,
      action: { kind: 'command', commandId: 'editor.pick_project' },
      preview: 'Open a project directory without typing a path.'
    },
    {
      id: 'project-action:open-fun',
      type: 'project',
      title: 'Open Fun Project',
      subtitle: state.status?.target_fps_path ?? state.projectPath,
      score: projectQuery ? 86 : 116,
      scope: activeScope,
      action: { kind: 'command', commandId: 'editor.open_default_project' },
      preview: 'Switch back to the canonical Fun workspace.'
    }
  ];

  if (state.projectPath && !state.projectPath.includes(state.status?.target_fps_path ?? '\0')) {
    results.push({
      id: 'project-action:open-typed-path',
      type: 'project',
      title: 'Open Current Project Path',
      subtitle: state.projectPath,
      score: projectQuery ? 82 : 112,
      scope: activeScope,
      action: { kind: 'open_project', path: state.projectPath },
      preview: 'Open the path currently held by the editor project state.'
    });
  }

  const seen = new Set<string>();
  const addProject = (
    project: { id: string; display_name: string; root_path: string },
    label: 'Current project' | 'Recent project' | 'Authorized project',
    baseScore: number
  ): void => {
    const key = `${project.id}:${project.root_path}`;
    if (seen.has(key)) {
      return;
    }
    seen.add(key);
    results.push({
      id: `project-switch:${label.toLowerCase().replace(/\s+/g, '-')}:${project.id}`,
      type: 'project',
      title: `Switch to ${project.display_name}`,
      subtitle: `${label} - ${project.root_path}`,
      score: baseScore,
      scope: { type: 'current_project', projectId: project.id },
      action: { kind: 'open_project', path: project.root_path },
      preview: label
    });
  };

  if (state.project) {
    addProject(
      { id: state.project.id, display_name: state.project.display_name, root_path: state.project.root_path },
      'Current project',
      projectQuery ? 78 : 108
    );
  }
  for (const project of state.recentProjects) {
    addProject(project, 'Recent project', projectQuery ? 76 : 106);
  }
  for (const project of state.projects) {
    addProject(project, 'Authorized project', projectQuery ? 72 : 102);
  }

  return results.map((result) => ({
    ...result,
    score: result.score + scoreQuery(projectQuery, result.title, result.subtitle, result.preview, 'project workspace switch recent')
  }));
}

function diagnosticSeverity(level: Diagnostic['level']): DiagnosticSeverity {
  if (level === 'warning') {
    return 'warn';
  }
  return level;
}

function diagnosticTags(diagnostic: Diagnostic): string[] {
  const tags = new Set<string>();
  for (const part of diagnostic.code.split(/[._:-]/).filter(Boolean)) {
    tags.add(part.toLowerCase());
  }
  const target = diagnostic.target_path ?? diagnostic.hosted_instance_id ?? '';
  if (/server/i.test(target) || /server/i.test(diagnostic.code)) {
    tags.add('server');
  }
  if (/client/i.test(target) || /client/i.test(diagnostic.code)) {
    tags.add('client');
  }
  if (/preview|viewport|render/i.test(diagnostic.code)) {
    tags.add('preview');
  }
  if (/build|cargo|compile/i.test(diagnostic.message)) {
    tags.add('build');
  }
  if (/material|shader|wgsl/i.test(diagnostic.message) || /material|shader|wgsl/i.test(diagnostic.code)) {
    tags.add('material');
  }
  return [...tags];
}
