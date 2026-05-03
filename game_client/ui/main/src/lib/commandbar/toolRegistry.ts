import type {
  CommandbarResult,
  EditorToolDefinition,
  EditorUiState,
  ExperienceScope,
  HostCommandDescriptor,
  ToolCall,
  ToolCallPreview
} from '../types';
import { scopeTargetLabel } from './scopeRegistry';

const builtinTools: EditorToolDefinition[] = [
  tool('project.current.get', 'Current Project', 'Read the active project identity and roots.', 'project', 'read', ['project', 'workspace', 'current']),
  tool('projects.recent.list', 'Recent Projects', 'List recent projects and project switch targets.', 'project', 'read', ['recent', 'project', 'switch']),
  tool('project.bsn.index', 'BSN Scene Index', 'Search indexed BSN scenes and invocations.', 'scene', 'read', ['scene', 'bsn', 'index']),
  tool('entity.search', 'Search Entities', 'Search static and live ECS entity rows.', 'entity', 'read', ['entity', 'outliner', 'component']),
  tool('entity.details.get', 'Inspect Entity', 'Read details for the selected entity.', 'entity', 'read', ['inspect', 'entity', 'component']),
  tool('runtime.status.get', 'Runtime Status', 'Read client, server, and host runtime state.', 'runtime', 'read', ['runtime', 'client', 'server']),
  tool('runtime.diagnostics.list', 'Runtime Diagnostics', 'Read editor and runtime diagnostics.', 'diagnostics', 'read', ['diagnostic', 'log', 'warning', 'error']),
  tool('preview.renderer.status.get', 'Preview State', 'Read current-client preview state.', 'preview', 'read', ['preview', 'render', 'status']),
  tool('material.shader.list', 'Material Shaders', 'Search WGSL material shader surfaces.', 'material', 'read', ['material', 'shader', 'wgsl', 'graph']),
  tool('editor.events.list', 'Editor Event Log', 'Read recent editor activity.', 'diagnostics', 'read', ['event', 'log', 'activity']),
  commandTool('context.set.overview', 'Open Overview', 'Open the project, directory, entity, and asset overview.', 'project', 'editor.set_context.overview', ['overview', 'outliner', 'directory']),
  commandTool('context.set.preview', 'Open Viewport', 'Switch to the current-client preview surface.', 'scene', 'editor.set_context.preview', ['preview', 'viewport', 'current client']),
  commandTool('context.set.graph', 'Open Material Graph', 'Switch to the material graph overlay.', 'material', 'editor.set_context.graph', ['graph', 'material', 'shader']),
  commandTool('context.set.diagnostics', 'Open Log', 'Open the combined diagnostics, runtime, event, and command log.', 'diagnostics', 'editor.set_context.diagnostics', ['log', 'diagnostics', 'runtime trace', 'command results']),
  commandTool('viewport.fit', 'Frame Viewport', 'Frame the active editor viewport.', 'preview', 'editor.frame_viewport', ['frame', 'fit', 'viewport']),
  commandTool('preview.renderer.reload', 'Reload Preview', 'Refresh the current-client preview state.', 'preview', 'editor.reload_preview', ['reload', 'preview', 'render']),
  commandTool('viewport.client.launch', 'Play Client', 'Switch to gameplay in the unified host.', 'runtime', 'editor.launch_client', ['play', 'client', 'game']),
  commandTool('runtime.server.launch', 'Launch Server', 'Start the selected project server runtime.', 'runtime', 'editor.launch_server', ['server', 'launch', 'runtime']),
  commandTool('viewport.client.stop', 'Clear Client Focus', 'Leave the current client running and clear editor focus.', 'runtime', 'editor.stop_runtime', ['client', 'runtime']),
  tool('entity.transform.patch', 'Patch Entity Transform', 'Change selected entity transform through a typed undoable write.', 'entity', 'write', ['move', 'transform', 'position'], true),
  tool('material.shader.save', 'Save Material Shader', 'Persist a material shader as standard WGSL.', 'material', 'write', ['material', 'shader', 'save', 'wgsl'], true),
  tool('preview.renderer.scene.set', 'Set Preview Scene', 'Change the exact preview scene.', 'preview', 'write', ['scene', 'preview', 'set'], true),
  tool('runtime.inspector.attach', 'Attach Runtime Inspector', 'Attach to a client or server inspector.', 'runtime', 'network', ['inspector', 'attach', 'client', 'server'], true),
  tool('backend.experience.host', 'Host This Experience', 'Plan a hosted experience lifecycle action.', 'backend', 'network', ['host', 'backend', 'experience'], true, 'Backend hosting commands are not connected yet.'),
  tool('backend.experience.publish', 'Publish Experience', 'Plan a backend publish action.', 'backend', 'admin', ['publish', 'deploy', 'backend'], true, 'Backend publishing is intentionally gated.'),
  tool('backend.experience.sync', 'Sync Experience Assets', 'Plan a backend asset sync action.', 'backend', 'network', ['sync', 'cdn', 'assets'], true, 'Backend asset sync is not connected yet.'),
  tool('llm.reason_over_scope', 'Ask LLM Over Scope', 'Reason over retrieved project, runtime, and diagnostic context.', 'mcp', 'read', ['ask', 'llm', 'explain', 'why'])
];

function tool(
  id: string,
  title: string,
  description: string,
  category: EditorToolDefinition['category'],
  risk: EditorToolDefinition['risk'],
  keywords: string[],
  requiresConfirmation = risk !== 'read',
  disabledReason?: string
): EditorToolDefinition {
  return {
    id,
    title,
    description,
    category,
    scopeKinds: ['current_project', 'project_client_server', 'all_projects', 'all_connected', 'client', 'server'],
    risk,
    requiresConfirmation,
    keywords,
    disabledReason
  };
}

function commandTool(
  id: string,
  title: string,
  description: string,
  category: EditorToolDefinition['category'],
  commandId: string,
  keywords: string[]
): EditorToolDefinition {
  return {
    ...tool(id, title, description, category, 'read', keywords, false),
    commandId
  };
}

export function editorToolDefinitions(state: EditorUiState): EditorToolDefinition[] {
  const registeredCommandTools = state.commands.map<EditorToolDefinition>((command) => ({
    id: `command.${command.id}`,
    title: command.title,
    description: command.summary,
    category: 'launcher',
    scopeKinds: ['current_project', 'project_client_server', 'all_projects', 'all_connected'],
    risk: toolRiskFromHostCommand(command),
    requiresConfirmation: command.risk?.requires_confirmation ?? false,
    keywords: [
      command.id,
      command.category,
      command.input_description,
      command.output_description,
      command.payload_schema_id ?? '',
      command.required_capability ?? '',
      command.risk?.level ?? ''
    ].filter(Boolean),
    commandId: command.id
  }));
  return [...builtinTools, ...registeredCommandTools];
}

function toolRiskFromHostCommand(command: HostCommandDescriptor): EditorToolDefinition['risk'] {
  const level = command.risk?.level ?? 'read';
  if (level === 'account' || level === 'network') {
    return 'network';
  }
  if (level === 'filesystem' || level === 'runtime_mutation' || level === 'ui_navigation') {
    return 'write';
  }
  if (level === 'tool_stub') {
    return 'admin';
  }
  return 'read';
}

export function commandbarResultFromTool(
  definition: EditorToolDefinition,
  scope: ExperienceScope,
  query: string,
  score: number
): CommandbarResult {
  const action = definition.commandId
    ? { kind: 'command' as const, commandId: definition.commandId }
    : { kind: 'tool_call_preview' as const, toolCall: toolCallFromDefinition(definition, scope, { query }) };
  return {
    id: `tool:${definition.id}`,
    type: definition.commandId ? 'command' : 'tool',
    title: definition.title,
    subtitle: definition.disabledReason ?? definition.description,
    score,
    scope,
    action,
    preview: definition.description,
    metadata: {
      category: definition.category,
      risk: definition.risk,
      keywords: definition.keywords,
      disabledReason: definition.disabledReason
    }
  };
}

export function toolCallFromDefinition(
  definition: EditorToolDefinition,
  scope: ExperienceScope,
  args: Record<string, unknown>
): ToolCall {
  return {
    id: `${definition.id}:${stableToken(JSON.stringify(args))}`,
    toolName: definition.id,
    scope,
    arguments: args,
    risk: definition.risk,
    requiresConfirmation: definition.requiresConfirmation
  };
}

export function toolCallPreview(call: ToolCall): ToolCallPreview {
  return {
    call,
    title: call.toolName,
    summary:
      call.risk === 'read'
        ? 'Read-only commandbar tool call routed through the editor tool registry.'
        : 'This typed tool call can change editor, project, runtime, or network state and must be confirmed by the editor.',
    affectedEntities: [scopeTargetLabel(call.scope)],
    confirmationLabel: call.requiresConfirmation ? 'Review and run' : undefined,
    cancelLabel: call.requiresConfirmation ? 'Cancel' : undefined
  };
}

function stableToken(value: string): string {
  let hash = 2166136261;
  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index);
    hash = Math.imul(hash, 16777619);
  }
  return (hash >>> 0).toString(36);
}
