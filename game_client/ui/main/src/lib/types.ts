export type * from './types.generated';

import type {
  AccountProfileSummary,
  BackendAuthTicket,
  CommandDescriptor,
  CommandResult,
  ClientBuildProfile,
  Diagnostic,
  EditorEvent,
  EditorStatus,
  EntityDetails,
  EntityFilter,
  EntityRowSummary,
  EntitySourceFilter,
  EntityStreamCursor,
  ProjectReference,
  ProjectSummary,
  RuntimeInstanceSummary,
  RuntimeHostStatus,
  RuntimeDiagnosticsSnapshot,
  RuntimeStatusSummary,
  ViewportInstanceSummary,
  AuthorizedProject,
  JoinableGame,
  LauncherActiveRuntime,
  LauncherAuthorization,
  LauncherMode,
  PreviewRendererStatus,
  ViewportRect
} from './types.generated';

export type DomainFilter = EntityFilter['domain'] | 'all';
export type SourceFilter = EntitySourceFilter;
export type BuildProfile = ClientBuildProfile;
export type RenderMode = 'solari' | 'meshlets' | 'raster_fallback';
export type LauncherTab = 'games' | 'projects' | 'settings';
export type EditorContextId = 'overview' | 'preview' | 'live_client' | 'server' | 'diagnostics' | 'graph';
export type EditorPaneId = 'overview' | 'details' | 'viewport' | 'graph' | 'log';
export type EditorPaneDockZone = 'left' | 'center' | 'right' | 'bottom';
export type PreviewLifecycleStatus = 'idle' | 'preparing' | 'ready' | 'failed';
export type DiagnosticSeverity = 'error' | 'warn' | 'info' | 'trace';
export type DiagnosticsSortMode = 'newest' | 'oldest' | 'severity' | 'source' | 'file';
export type DiagnosticsFilterMode = 'any' | 'all';
export type CommandbarMode = 'identity' | 'search' | 'command' | 'ask' | 'tool' | 'project' | 'remote';
export type CommandbarModelRoute = 'none' | 'local' | 'cloud' | 'mcp';
export type CommandbarIntentKind =
  | 'open'
  | 'search'
  | 'inspect'
  | 'diagnose'
  | 'run'
  | 'mutate'
  | 'navigate'
  | 'backend'
  | 'tool'
  | 'ask';

export type ExperienceScope =
  | { type: 'current_project'; projectId: string }
  | { type: 'all_projects' }
  | { type: 'client'; clientId: string }
  | { type: 'server'; serverId: string }
  | { type: 'project_client_server'; projectId: string; clientId?: string; serverId?: string }
  | { type: 'all_connected' };

export type CommandbarAction =
  | { kind: 'command'; commandId: string }
  | { kind: 'open_project'; path: string }
  | { kind: 'set_context'; contextId: EditorContextId }
  | { kind: 'filter_diagnostics'; severity?: DiagnosticSeverity; tag?: string }
  | { kind: 'select_entity'; entityId: string }
  | { kind: 'tool_call_preview'; toolCall: ToolCall }
  | { kind: 'noop' };

export type ToolCallRisk = 'read' | 'write' | 'destructive' | 'network' | 'admin';
export type HostCommandRiskLevel =
  | 'read'
  | 'ui_navigation'
  | 'runtime_mutation'
  | 'filesystem'
  | 'network'
  | 'account'
  | 'tool_stub';

export interface HostCommandRiskMetadata {
  level: HostCommandRiskLevel;
  mutates_state: boolean;
  requires_confirmation: boolean;
  filesystem_access: boolean;
  network_access: boolean;
  tool_stub: boolean;
}

export type HostCommandDescriptor = CommandDescriptor & {
  payload_schema_id?: string;
  required_capability?: string;
  risk?: HostCommandRiskMetadata;
  rust_owner?: string;
};

export interface ToolCall {
  id: string;
  toolName: string;
  scope: ExperienceScope;
  arguments: Record<string, unknown>;
  risk: ToolCallRisk;
  requiresConfirmation: boolean;
}

export interface ToolCallPreview {
  call: ToolCall;
  title: string;
  summary: string;
  affectedEntities: string[];
  confirmationLabel?: string;
  cancelLabel?: string;
}

export interface CommandbarResult {
  id: string;
  type:
    | 'command'
    | 'file'
    | 'entity'
    | 'asset'
    | 'diagnostic'
    | 'project'
    | 'client'
    | 'server'
    | 'tool'
    | 'llm_answer';
  title: string;
  subtitle?: string;
  icon?: string;
  score: number;
  scope: ExperienceScope;
  action?: CommandbarAction;
  preview?: string;
  metadata?: Record<string, unknown>;
}

export interface CommandbarIntent {
  query: string;
  intent: CommandbarIntentKind;
  confidence: number;
  scopes: ExperienceScope[];
  entities: Array<{
    kind: string;
    name: string;
    confidence: number;
  }>;
  preferredToolIds: string[];
  needsLlm: boolean;
  riskCeiling: ToolCallRisk;
}

export interface CommandbarContextSnapshot {
  projectId?: string;
  projectName?: string;
  activeSceneId?: string | null;
  selectedEntityId?: string | null;
  activeEditorContextId: EditorContextId;
  clientId?: string;
  serverId?: string;
  diagnosticCount: number;
  commandCount: number;
}

export type EditorToolCategory =
  | 'project'
  | 'scene'
  | 'entity'
  | 'material'
  | 'preview'
  | 'runtime'
  | 'diagnostics'
  | 'launcher'
  | 'backend'
  | 'mcp';

export interface EditorToolDefinition {
  id: string;
  title: string;
  description: string;
  category: EditorToolCategory;
  scopeKinds: ExperienceScope['type'][];
  risk: ToolCallRisk;
  requiresConfirmation: boolean;
  keywords: string[];
  commandId?: string;
  disabledReason?: string;
}

export interface ExperienceRecord {
  id: string;
  kind: 'project' | 'client' | 'server' | 'preview' | 'backend_experience' | 'user_hosted_experience';
  title: string;
  subtitle?: string;
  status: 'ready' | 'running' | 'stopped' | 'connecting' | 'failed';
  scope: ExperienceScope;
  capabilities: string[];
  tools: string[];
  lastSeenAt: number;
}

export interface DiagnosticsFilterState {
  severities: DiagnosticSeverity[];
  tags: string[];
  sources: string[];
  scopes: string[];
  currentFileOnly: boolean;
}

export interface DiagnosticsViewState {
  visible: boolean;
  height: number;
  lastFocusedElementId?: string;
  unreadCriticalCount: number;
  unreadWarningCount: number;
  unreadInfoCount: number;
  activeFilters: DiagnosticsFilterState;
  filterMode: DiagnosticsFilterMode;
  sortMode: DiagnosticsSortMode;
  compact: true;
}

export interface CommandbarState {
  focused: boolean;
  input: string;
  mode: CommandbarMode;
  inferredIntent?: string;
  activeScope: ExperienceScope;
  selectedResultId?: string;
  results: CommandbarResult[];
  pendingToolCall?: ToolCallPreview;
  modelRoute?: CommandbarModelRoute;
}

export interface AccountUiState {
  loginOpen: boolean;
  loading: boolean;
  backendUrl: string;
  profile: AccountProfileSummary | null;
  ticket: BackendAuthTicket | null;
  error: string | null;
}

export interface SearchIndexRecord {
  id: string;
  scopeId: string;
  entityType: string;
  title: string;
  subtitle?: string;
  path?: string;
  tags: string[];
  keywords: string[];
  references?: string[];
  lastUpdated: number;
  openAction?: CommandbarAction;
  inspectAction?: CommandbarAction;
}

export type OverviewItemKind =
  | 'project'
  | 'directory'
  | 'crate'
  | 'target'
  | 'scene'
  | 'entity'
  | 'asset'
  | 'diagnostic'
  | 'runtime'
  | 'command';

export interface OverviewSelection {
  kind: OverviewItemKind;
  id: string;
  title: string;
  subtitle?: string;
  path?: string;
  scope?: ExperienceScope;
  metadata: Record<string, string | number | boolean | null>;
}

export interface OverviewState {
  query: string;
  selectedItem: OverviewSelection | null;
}

export interface EditorContextSummary {
  id: EditorContextId;
  label: string;
  status: string;
}

export interface PreviewRenderStatus {
  status: PreviewLifecycleStatus;
  sceneId: string | null;
  frameIndex: number;
  warning: string | null;
  error: string | null;
  renderer: PreviewRendererStatus | null;
}

export interface EntityViewportState {
  cursor: EntityStreamCursor | null;
  rowsByIndex: Record<number, EntityRowSummary>;
  totalRows: number;
  pageSize: number;
  overscanRows: number;
  loading: boolean;
  query: string;
  domain: DomainFilter;
  source: SourceFilter;
  selectedId: string | null;
  selectedDetails: EntityDetails | null;
}

export interface EditorUiState {
  launcherMode: LauncherMode;
  hostMode: string | null;
  hostInputOwner: string | null;
  games: JoinableGame[];
  projects: AuthorizedProject[];
  selectedGameId: string | null;
  selectedProjectId: string | null;
  activeRuntime: LauncherActiveRuntime | null;
  authorization: LauncherAuthorization;
  runtimeHostStatus: RuntimeHostStatus | null;
  status: EditorStatus | null;
  runtimeStatus: RuntimeStatusSummary | null;
  commands: HostCommandDescriptor[];
  events: EditorEvent[];
  project: ProjectSummary | null;
  recentProjects: ProjectReference[];
  viewport: ViewportInstanceSummary | null;
  viewportRect: ViewportRect | null;
  editorContexts: EditorContextSummary[];
  activeEditorContextId: EditorContextId;
  editorContext: EditorContextId;
  availableContexts: EditorContextSummary[];
  contextSwitching: boolean;
  previewStatus: PreviewLifecycleStatus;
  previewError: string | null;
  previewFrameRevision: number;
  previewSceneId: string | null;
  previewRenderStatus: PreviewRenderStatus;
  liveClientStatus: ViewportInstanceSummary | null;
  serverRuntime: RuntimeInstanceSummary | null;
  overview: OverviewState;
  entities: EntityViewportState;
  diagnostics: Diagnostic[];
  runtimeDiagnostics: RuntimeDiagnosticsSnapshot | null;
  diagnosticsView: DiagnosticsViewState;
  account: AccountUiState;
  commandbar: CommandbarState;
  projectPath: string;
  activeSceneId: string | null;
  commandSearch: string;
  buildProfile: BuildProfile;
  renderMode: RenderMode;
  bottomTab: 'diagnostics' | 'event_log' | 'frame_profile' | 'runtime_trace' | 'command_results';
  lastCommandResult: CommandResult<unknown> | null;
  loading: boolean;
  opening: boolean;
  launchingClient: boolean;
  launchingServer: boolean;
  stoppingClient: boolean;
  error: string | null;
}
