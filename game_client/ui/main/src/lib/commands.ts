import { invokeHost, isHostRuntime as hasHostCommandTransport } from './host/commands';
import type {
  AuthSessionSummary,
  BackendAccountTicketLoginRequest,
  BackendAuthSessionState,
  BackendAuthTicket,
  BackendAuthTicketRequest,
  BsnIndexSummary,
  BuildProfile,
  CommandDescriptor,
  CommandResult,
  ComponentSummary,
  Diagnostic,
  EcsDomain,
  EditorEvent,
  EditorStatus,
  EntityDetails,
  EntityFilter,
  EntityRowSummary,
  EntitySourceKind,
  EntityStreamCloseSummary,
  EntityStreamCursor,
  EntityStreamOpenRequest,
  EntityStreamPage,
  EntityStreamPageRequest,
  HostedInstanceSummary,
  AuthorizedProject,
  JoinableGame,
  LauncherMode,
  LauncherState,
  RuntimeHostStatus,
  LiveEntityStreamOpenRequest,
  MaterialShaderCatalog,
  MaterialShaderDocument,
  MaterialShaderLoadRequest,
  MaterialShaderSaveRequest,
  ProjectCrate,
  ProjectReference,
  ProjectSummary,
  PreviewRendererStatus,
  RuntimeAttachRequest,
  RuntimeDiagnosticsSnapshot,
  RuntimeInstanceSummary,
  RuntimeStatusSummary,
  TransformPatchRequest,
  TransformPatchResult,
  SourcePreview,
  ViewportInstanceSummary,
  ViewportRect
} from './types';
import type { CommandbarAction, HostCommandDescriptor } from './types';


export const DEFAULT_ENTITY_PAGE_SIZE = 200;
export const DEFAULT_ENTITY_OVERSCAN = 45;
export const MOCK_ENTITY_COUNT =
  import.meta.env.DEV && import.meta.env.VITE_FUN_EDITOR_MOCK_DATA === '1' ? 50_000 : 0;

export type FunHostMode = 'boot' | 'launcher' | 'game' | 'editor' | 'editor_overlay' | 'loading' | 'shutdown';
export type FunInputOwner = 'gameplay' | 'launcher_ui' | 'editor_ui' | 'game_menu_ui' | 'text_entry' | 'commandbar';

export interface FunHostState {
  mode: FunHostMode;
  input_owner: FunInputOwner;
  game: {
    simulation_active: boolean;
    hud_passive: boolean;
    render_visible_behind_ui: boolean;
    active_game_id?: string | null;
    active_session_id?: string | null;
    viewport_layout: {
      revision: number;
      rect: {
        x: number;
        y: number;
        width: number;
        height: number;
        scale_factor_milli: number;
      } | null;
    };
  };
  launcher: {
    visible: boolean;
    input_capture: boolean;
    selected_game_id: string | null;
    selected_project_id: string | null;
  };
  editor: {
    active: boolean;
    overlay_active: boolean;
    input_capture: boolean;
    services_active: boolean;
    active_route?: 'overview' | 'preview' | 'live_client' | 'server' | 'graph' | 'diagnostics';
    preview_mode: 'current_client' | 'preview_world';
  };
  project: {
    local_project_authorized: boolean;
    active_project_id: string | null;
    active_project_root: string | null;
  };
  runtime: {
    return_to_game_available: boolean;
    active_runtime_id: string | null;
    loading_visible: boolean;
    server_addr?: string | null;
    client_session_configured?: boolean;
    server_runtime_running?: boolean;
  };
  account?: {
    backend_session_available: boolean;
    ticket_redacted: boolean;
  };
}

export interface FunHostSnapshotPayload {
  snapshot: {
    product_host: 'FunClientHost';
    lifecycle: 'booting' | 'ready' | 'shutting_down';
    state: FunHostState;
    previous_mode: FunHostMode | null;
    native_ui_command_surface_ready: boolean;
  };
  command_catalog: HostCommandDescriptor[];
  editor_command_catalog: CommandDescriptor[];
}

const targetPath = 'C:\\Users\\premi\\work\\project-FUN\\fun';

const expectedCrates = [
  'game_client',
  'game_server',
  'game_shared',
  'game_scene',
  'game_launcher'
];

const fallbackBsnIndex: BsnIndexSummary = {
  revision: 1,
  project_root: targetPath,
  indexed_roots: [
    `${targetPath}\\game_server\\src`,
    `${targetPath}\\game_client\\src`,
    `${targetPath}\\game_shared\\src`,
    `${targetPath}\\game_scene\\src`
  ],
  records: [
    {
      id: 'fallback-default-scene',
      invocation_kind: 'bsn_list',
      domain: 'shared',
      file_path: `${targetPath}\\game_scene\\src\\lib.rs`,
      span: {
        path: `${targetPath}\\game_scene\\src\\lib.rs`,
        byte_start: 0,
        byte_end: 0,
        start: { line: 92, column: 5 },
        end: { line: 168, column: 6 }
      },
      scene_function_name: 'spawn_default_scene',
      detected_names: ['#Floor', '#FloorCollider', '#Wall', '#Ramp'],
      component_type_tokens: ['Collider', 'Name', 'RigidBody', 'StreamedWorldEntity', 'Transform']
    }
  ],
  record_count: 1,
  detected_scene_count: 1
};

export const fallbackProject: ProjectSummary = {
  id: 'project-fun-browser-preview',
  root_path: targetPath,
  display_name: 'fun',
  crates: [
    crate('game_client', 'client', 'client'),
    crate('game_server', 'server', 'server'),
    crate('game_shared', 'shared', 'shared'),
    crate('game_scene', 'support', 'shared'),
    crate('game_launcher', 'launcher', null)
  ],
  targets: expectedCrates,
  asset_roots: [],
  bsn_roots: [],
  bsn_index: fallbackBsnIndex,
  diagnostics: [
    diagnostic(
      'host.runtime_missing',
      'warning',
      'Browser preview cannot inspect the filesystem. Open the Fun host for live project data.'
    )
  ],
  current_viewport_status: null
};

export const fallbackRuntimeStatus: RuntimeStatusSummary = {
  client_process: status('stopped', 'client process is stopped', false),
  client_inspector: status('stopped', 'client inspector is disconnected', false),
  server_process: status('stopped', 'server process is stopped', false),
  server_inspector: status('stopped', 'server inspector is disconnected', false),
  game_network: status('stopped', 'client/server game network is independent from editor auth', false),
  update_rate_limit_hz: 20
};

const fallbackAuth: AuthSessionSummary = {
  session_id: 'browser-preview',
  control_addr: 'browser-preview',
  token_redacted: true,
  allowed_capabilities: [
    'read_entities',
    'read_components',
    'read_resources',
    'read_diagnostics',
    'control_runtime',
    'mutate_entities',
    'apply_scene_patch',
    'execute_server_code',
    'execute_client_code',
    'persist_iteration'
  ],
  expires_unix_ms: Date.now() + 60 * 60 * 1000,
  expired: false
};

export const fallbackStatus: EditorStatus = {
  stage_name: 'Stage 1: Running Project Host',
  target_fps_path: targetPath,
  fun_workspace_exists: false,
  expected_fun_crates: expectedCrates,
  ui_stack: 'Native rvelte/FUN packets with strict TypeScript browser preview',
  rust_service_status: 'fallback browser preview',
  retired_engine_demo_status: 'requires Fun-hosted Rust command',
  current_project: null,
  client_viewport_status: 'requires Fun host',
  runtime_status: fallbackRuntimeStatus,
  auth_session: fallbackAuth
};

const fallbackGames: JoinableGame[] = [
  {
    id: 'local-fun-dev',
    title: 'Game #1',
    summary: 'First Fun experience slot.',
    region: 'local',
    status: 'local',
    players_online: 1,
    max_players: 16,
    latency_ms: 0,
    project_id: fallbackProject.id,
    build_profile: 'debug',
    accent: 'gold'
  },
  {
    id: 'preview-renderer',
    title: 'Game #2',
    summary: 'Second Fun experience slot.',
    region: 'editor',
    status: 'local',
    players_online: 0,
    max_players: 1,
    latency_ms: null,
    project_id: fallbackProject.id,
    build_profile: 'debug',
    accent: 'amber'
  }
];

const fallbackProjects: AuthorizedProject[] = [
  {
    id: fallbackProject.id,
    display_name: fallbackProject.display_name,
    root_path: fallbackProject.root_path,
    crate_count: fallbackProject.crates.length,
    bsn_scene_count: fallbackProject.bsn_index.detected_scene_count,
    edit_authorized: true,
    authorization_summary: 'browser preview authorization',
    last_opened_label: 'available'
  }
];

export const fallbackRuntimeHostStatus: RuntimeHostStatus = {
  launcher_mode: 'launcher',
  host_state: 'browser_preview',
  active_runtime: null,
  runtime_status: fallbackRuntimeStatus
};

const fallbackLauncherState: LauncherState = {
  launcher_mode: 'launcher',
  games: fallbackGames,
  projects: fallbackProjects,
  selected_game_id: fallbackGames[0].id,
  selected_project_id: fallbackProjects[0].id,
  active_runtime: null,
  authorization: {
    can_join_game: false,
    can_host_local: false,
    can_open_project: true,
    can_edit_project: true,
    can_resume_editor: false,
    can_stop_runtime: false,
    capabilities: fallbackAuth.allowed_capabilities,
    reason: 'Browser preview cannot control the Fun host runtime.'
  },
  runtime_host_status: fallbackRuntimeHostStatus
};

export let browserFallbackLauncherState: LauncherState = fallbackLauncherState;

function fallbackFunHostState(): FunHostState {
  return {
    mode:
      browserFallbackLauncherState.launcher_mode === 'editor'
        ? 'editor'
        : browserFallbackLauncherState.launcher_mode === 'launcher'
          ? 'launcher'
          : 'game',
    input_owner:
      browserFallbackLauncherState.launcher_mode === 'editor'
        ? 'editor_ui'
        : browserFallbackLauncherState.launcher_mode === 'launcher'
          ? 'launcher_ui'
          : 'gameplay',
    game: {
      simulation_active: browserFallbackLauncherState.launcher_mode === 'hidden',
      hud_passive: browserFallbackLauncherState.launcher_mode === 'hidden',
      render_visible_behind_ui: true,
      viewport_layout: {
        revision: 0,
        rect: null
      }
    },
    launcher: {
      visible: browserFallbackLauncherState.launcher_mode === 'launcher',
      input_capture: browserFallbackLauncherState.launcher_mode === 'launcher',
      selected_game_id: browserFallbackLauncherState.selected_game_id,
      selected_project_id: browserFallbackLauncherState.selected_project_id
    },
    editor: {
      active: browserFallbackLauncherState.launcher_mode === 'editor',
      overlay_active: false,
      input_capture: browserFallbackLauncherState.launcher_mode === 'editor',
      services_active: browserFallbackLauncherState.launcher_mode === 'editor',
      preview_mode: 'current_client'
    },
    project: {
      local_project_authorized: browserFallbackLauncherState.authorization.can_open_project,
      active_project_id: browserFallbackLauncherState.selected_project_id,
      active_project_root: fallbackProject.root_path
    },
    runtime: {
      return_to_game_available: true,
      active_runtime_id: browserFallbackLauncherState.active_runtime?.instance_id ?? null,
      loading_visible: false
    }
  };
}

function setBrowserFallbackLauncherMode(
  launcherMode: LauncherState['launcher_mode'],
  selectedProjectId = browserFallbackLauncherState.selected_project_id
): LauncherState {
  browserFallbackLauncherState = {
    ...browserFallbackLauncherState,
    launcher_mode: launcherMode,
    selected_project_id: selectedProjectId,
    runtime_host_status: {
      ...browserFallbackLauncherState.runtime_host_status,
      launcher_mode: launcherMode
    }
  };
  return browserFallbackLauncherState;
}

export const fallbackCommands: HostCommandDescriptor[] = [
  descriptor('host.commands.list', 'List Host Commands', 'Returns Rust-owned host command descriptors.', 'host', 'No input.', 'HostCommandDescriptor[]'),
  descriptor('host.commandbar.execute', 'Execute Commandbar Action', 'Validates commandbar execution through Rust host authority.', 'host', 'Commandbar action.', 'CommandResult<CommandbarExecutionPayload>'),
  descriptor('launcher.state.get', 'Get Launcher State', 'Returns launcher state.', 'launcher', 'No input.', 'LauncherState'),
  descriptor('launcher.show', 'Show Launcher', 'Switches to launcher mode.', 'launcher', 'No input.', 'CommandResult<LauncherState>'),
  descriptor('launcher.hide', 'Hide Launcher', 'Returns input to gameplay in the unified host.', 'launcher', 'No input.', 'CommandResult<LauncherState>'),
  descriptor('games.list', 'List Games', 'Returns join targets.', 'launcher', 'No input.', 'JoinableGame[]'),
  descriptor('games.join', 'Join Game', 'Switches the current host to game mode.', 'launcher', 'Game ID.', 'CommandResult<LauncherState>'),
  descriptor('projects.authorized.list', 'List Authorized Projects', 'Returns authorized projects.', 'launcher', 'No input.', 'AuthorizedProject[]'),
  descriptor('project.edit.open', 'Open Project Editor', 'Opens the selected project in editor mode.', 'launcher', 'Project ID.', 'CommandResult<LauncherState>'),
  descriptor('editor.activate', 'Activate Editor', 'Switches into the editor shell.', 'launcher', 'No input.', 'CommandResult<LauncherState>'),
  descriptor('editor.deactivate', 'Deactivate Editor', 'Returns input to gameplay.', 'launcher', 'No input.', 'CommandResult<LauncherState>'),
  descriptor('editor.overlay.toggle', 'Toggle Editor Overlay', 'Toggles the local in-process editor overlay.', 'launcher', 'No input.', 'CommandResult<LauncherState>'),
  descriptor('runtime.host.status', 'Get Runtime Host Status', 'Returns runtime host state.', 'launcher', 'No input.', 'RuntimeHostStatus'),
  descriptor('editor.status.get', 'Get Editor Status', 'Returns editor service status.', 'editor', 'No input.', 'EditorStatus'),
  descriptor('editor.commands.list', 'List Commands', 'Returns the searchable command registry.', 'editor', 'No input.', 'CommandDescriptor[]'),
  descriptor('editor.events.list', 'List Events', 'Returns the in-memory event log.', 'editor', 'No input.', 'EditorEvent[]'),
  descriptor('project.open', 'Open Project', 'Opens the Fun workspace.', 'project', 'Optional path.', 'CommandResult<ProjectSummary>'),
  descriptor('project.bsn.index', 'Index Fun Scene Sources', 'Returns fun! and fun_list! source records.', 'project', 'No input.', 'CommandResult<BsnIndexSummary>'),
  descriptor('material.shader.list', 'List Material Shaders', 'Lists native WGSL material shader files.', 'material', 'Project ID.', 'CommandResult<MaterialShaderCatalog>'),
  descriptor('material.shader.load', 'Load Material Shader', 'Reads a project-owned WGSL material shader file.', 'material', 'MaterialShaderLoadRequest.', 'CommandResult<MaterialShaderDocument>'),
  descriptor('material.shader.save', 'Save Material Shader', 'Writes a project-owned WGSL material shader file.', 'material', 'MaterialShaderSaveRequest.', 'CommandResult<MaterialShaderDocument>'),
  descriptor('entity_stream.open', 'Open Entity Stream', 'Creates a cursor for Rust-owned entity rows.', 'entity', 'EntityStreamOpenRequest.', 'CommandResult<EntityStreamCursor>'),
  descriptor('live_entity_stream.open', 'Open Live Entity Stream', 'Creates a cursor for authenticated live runtime rows.', 'entity', 'LiveEntityStreamOpenRequest.', 'CommandResult<EntityStreamCursor>'),
  descriptor('entity_stream.page', 'Page Entity Stream', 'Returns row summaries without component details.', 'entity', 'EntityStreamPageRequest.', 'CommandResult<EntityStreamPage>'),
  descriptor('entity.details.get', 'Get Entity Details', 'Fetches selected entity components and source preview.', 'entity', 'Entity ID.', 'CommandResult<EntityDetails>'),
  descriptor('entity.transform.patch', 'Patch Server Transform', 'Applies one server Transform mutation through the authenticated runtime protocol.', 'entity', 'TransformPatchRequest.', 'CommandResult<TransformPatchResult>'),
  descriptor('viewport.client.launch', 'Play Current Client', 'Switches the unified host to the current game client.', 'runtime', 'Project ID.', 'CommandResult<ViewportInstanceSummary>'),
  descriptor('preview.viewport.ensure', 'Use Current Client Preview', 'Compatibility command for the current-client editor preview.', 'runtime', 'Project ID and optional scene ID.', 'CommandResult<ViewportInstanceSummary>'),
  descriptor('preview.viewport.stop', 'Clear Preview Focus', 'Leaves current-client preview state intact.', 'runtime', 'Instance ID.', 'CommandResult<ViewportInstanceSummary>'),
  descriptor('preview.renderer.ensure', 'Use Current Client Preview', 'Binds editor preview to the current client render state.', 'runtime', 'Project ID, optional scene ID, optional rect.', 'CommandResult<PreviewRendererStatus>'),
  descriptor('preview.renderer.resize', 'Resize Preview Layout', 'Updates the editor overlay layout over the current client render.', 'runtime', 'Project ID, optional scene ID, rect.', 'CommandResult<PreviewRendererStatus>'),
  descriptor('preview.renderer.frame.get', 'Get Preview State', 'Returns current-client preview state metadata.', 'runtime', 'No input.', 'CommandResult<PreviewRendererStatus>'),
  descriptor('preview.renderer.scene.set', 'Set Preview Scene', 'Selects a scene in the current client preview state.', 'runtime', 'Project ID and scene ID.', 'CommandResult<PreviewRendererStatus>'),
  descriptor('preview.renderer.status.get', 'Get Preview State', 'Returns current-client preview state.', 'runtime', 'No input.', 'CommandResult<PreviewRendererStatus>'),
  descriptor('runtime.server.launch', 'Launch Server', 'Launches the server runtime.', 'runtime', 'Project ID.', 'CommandResult<RuntimeInstanceSummary>'),
  descriptor('runtime.status.get', 'Get Runtime Status', 'Returns independent runtime connection states.', 'runtime', 'No input.', 'RuntimeStatusSummary'),
  descriptor('runtime.diagnostics.list', 'List Runtime Diagnostics', 'Returns structured runtime diagnostic packet cache.', 'diagnostics', 'Optional RuntimeKind target.', 'CommandResult<RuntimeDiagnosticsSnapshot>'),
  descriptor('runtime.inspector.attach', 'Attach Runtime Inspector', 'Dials an explicitly enabled runtime inspector.', 'runtime', 'RuntimeAttachRequest.', 'CommandResult<RuntimeStatusSummary>'),
  descriptor('auth.session.get', 'Get Auth Session', 'Returns redacted auth session metadata.', 'auth', 'No input.', 'AuthSessionSummary'),
  descriptor('account.login', 'Account Login', 'Submits credentials to Rust-owned backend account authority.', 'auth', 'BackendAccountTicketLoginRequest.', 'CommandResult<BackendAuthTicket>'),
  descriptor('account.register', 'Account Register', 'Registers through Rust-owned backend account authority.', 'auth', 'BackendAccountTicketLoginRequest.', 'CommandResult<BackendAuthTicket>'),
  descriptor('account.logout', 'Account Logout', 'Clears the Rust-owned backend account session.', 'auth', 'No input.', 'CommandResult<BackendAuthSessionState>'),
  descriptor('auth.ticket.request', 'Request Backend Auth Ticket', 'Requests a scoped backend ticket from Rust-owned editor auth.', 'auth', 'BackendAuthTicketRequest.', 'CommandResult<BackendAuthTicket>'),
  descriptor('backend.auth.session.get', 'Get Backend Session', 'Returns redacted backend session state.', 'auth', 'No input.', 'BackendAuthSessionState')
];

export const fallbackEvents: EditorEvent[] = [
  {
    timestamp: 'browser-preview',
    command_id: 'editor.status.get',
    level: 'info',
    message: 'Running without Fun host. Rust project, process, and viewport commands need the desktop shell.',
    target_path: targetPath,
    hosted_instance_id: null
  }
];
let fallbackActiveFilter: EntityFilter | null = null;
let fallbackActiveSyntheticCount = MOCK_ENTITY_COUNT;

export async function getEditorStatus(): Promise<EditorStatus> {
  return isHostRuntime() ? invokeHost<EditorStatus>('editor.status.get') : fallbackStatus;
}

export async function getHostSnapshot(): Promise<FunHostSnapshotPayload> {
  return isHostRuntime()
    ? invokeHost<FunHostSnapshotPayload>('host.snapshot.get')
    : {
        snapshot: {
          product_host: 'FunClientHost',
          lifecycle: 'ready',
          state: fallbackFunHostState(),
          previous_mode: null,
          native_ui_command_surface_ready: false
        },
        command_catalog: fallbackCommands,
        editor_command_catalog: fallbackCommands
      };
}

async function invokeHostLauncherState(command: string, payload?: unknown): Promise<CommandResult<LauncherState>> {
  const snapshot = await invokeHost<FunHostSnapshotPayload>(command, payload ?? null);
  return { ok: true, value: launcherStateFromHostSnapshot(snapshot), diagnostics: [] };
}

async function invokeHostViewportState(
  command: string,
  payload: unknown,
  projectId?: string | null
): Promise<CommandResult<ViewportInstanceSummary>> {
  const snapshot = await invokeHost<FunHostSnapshotPayload>(command, payload ?? null);
  return { ok: true, value: currentClientViewportFromHostSnapshot(snapshot, projectId), diagnostics: [] };
}

async function invokeHostPreviewState(
  command: string,
  payload?: unknown
): Promise<CommandResult<PreviewRendererStatus>> {
  const snapshot = await invokeHost<FunHostSnapshotPayload>(command, payload ?? null);
  return { ok: true, value: previewStatusFromHostSnapshot(snapshot, payload), diagnostics: [] };
}

function launcherStateFromHostSnapshot(payload: FunHostSnapshotPayload): LauncherState {
  const hostState = payload.snapshot.state;
  const launcherMode = launcherModeForHostMode(hostState.mode);
  const selectedProjectId = hostState.launcher.selected_project_id ?? browserFallbackLauncherState.selected_project_id;
  const selectedGameId = hostState.launcher.selected_game_id ?? browserFallbackLauncherState.selected_game_id;
  const activeRuntime =
    hostState.game.simulation_active || hostState.mode === 'game' || hostState.mode === 'editor_overlay'
      ? {
          instance_id: hostState.runtime.active_runtime_id ?? 'current-client',
          project_id: selectedProjectId ?? fallbackProject.id,
          kind: 'client' as const,
          status: 'running' as const,
          pid: null,
          editor_owned: false,
          inspector_state: 'in_process',
          game_network_state: 'local'
        }
      : null;
  const runtimeHostStatus = {
    ...browserFallbackLauncherState.runtime_host_status,
    launcher_mode: launcherMode,
    host_state: hostState.mode,
    active_runtime: activeRuntime,
    runtime_status: fallbackRuntimeStatus
  } satisfies RuntimeHostStatus;
  return {
    ...browserFallbackLauncherState,
    launcher_mode: launcherMode,
    selected_game_id: selectedGameId,
    selected_project_id: selectedProjectId,
    active_runtime: activeRuntime,
    authorization: {
      ...browserFallbackLauncherState.authorization,
      can_join_game: true,
      can_host_local: true,
      can_open_project: hostState.project.local_project_authorized,
      can_edit_project: hostState.project.local_project_authorized,
      can_resume_editor: hostState.runtime.return_to_game_available,
      can_stop_runtime: false,
      reason: null
    },
    runtime_host_status: runtimeHostStatus
  };
}

function launcherModeForHostMode(mode: FunHostMode): LauncherMode {
  if (mode === 'editor' || mode === 'editor_overlay') {
    return 'editor';
  }
  if (mode === 'launcher' || mode === 'loading' || mode === 'boot') {
    return 'launcher';
  }
  return 'hidden';
}

function currentClientViewportFromHostSnapshot(
  payload: FunHostSnapshotPayload,
  projectId?: string | null
): ViewportInstanceSummary {
  const hostState = payload.snapshot.state;
  return {
    id: hostState.runtime.active_runtime_id ?? 'current-client',
    project_id: projectId ?? hostState.project.active_project_id ?? fallbackProject.id,
    kind: 'client',
    pid: null,
    status: 'running',
    window_handle_status: 'detached',
    inspector_addr: 'in-process',
    launch_command: ['fun-client', 'host.mode=game'],
    stdout_log_path: null,
    stderr_log_path: null,
    diagnostics: []
  };
}

function previewStatusFromHostSnapshot(
  payload: FunHostSnapshotPayload,
  requestPayload?: unknown
): PreviewRendererStatus {
  const hostState = payload.snapshot.state;
  const requested = requestPayload && typeof requestPayload === 'object' ? requestPayload as Record<string, unknown> : {};
  const rect = requested.rect && typeof requested.rect === 'object' ? requested.rect as Partial<ViewportRect> : null;
  const sceneId =
    typeof requested.sceneId === 'string'
      ? requested.sceneId
      : typeof requested.scene_id === 'string'
        ? requested.scene_id
        : fallbackBsnIndex.records[0]?.id ?? 'current-client-scene';
  const width = Math.max(1, Math.round(rect?.width ?? hostState.game.viewport_layout.rect?.width ?? 1280));
  const height = Math.max(1, Math.round(rect?.height ?? hostState.game.viewport_layout.rect?.height ?? 720));
  return {
    status: 'ready',
    scene: {
      scene_id: sceneId,
      entity_count: 0,
      revision: hostState.game.viewport_layout.revision,
      manifest_signature: hostState.game.viewport_layout.revision
    },
    frame: {
      frame_index: hostState.game.viewport_layout.revision,
      rgba_byte_len: 0,
      readback_status: 'current_client_background'
    },
    target: {
      width,
      height,
      format: 'CurrentClient',
      texture_usages: 'NATIVE_UI_OVERLAY_ONLY'
    },
    signature_warning: null
  };
}

export async function listEditorCommands(): Promise<HostCommandDescriptor[]> {
  return isHostRuntime() ? invokeHost<HostCommandDescriptor[]>('host.commands.list') : fallbackCommands;
}

export async function listEditorEvents(): Promise<EditorEvent[]> {
  return isHostRuntime() ? invokeHost<EditorEvent[]>('editor.events.list') : fallbackEvents;
}

export async function getLauncherState(): Promise<LauncherState> {
  if (isHostRuntime()) {
    const result = await invokeHostLauncherState('launcher.state.get');
    return result.value ?? browserFallbackLauncherState;
  }
  return browserFallbackLauncherState;
}

export async function showLauncher(): Promise<CommandResult<LauncherState>> {
  return isHostRuntime()
    ? invokeHostLauncherState('launcher.show')
    : { ok: true, value: setBrowserFallbackLauncherMode('launcher'), diagnostics: [] };
}

export async function hideLauncher(): Promise<CommandResult<LauncherState>> {
  return isHostRuntime()
    ? invokeHostLauncherState('launcher.hide')
    : { ok: true, value: setBrowserFallbackLauncherMode('hidden'), diagnostics: [] };
}

export async function listGames(): Promise<JoinableGame[]> {
  if (isHostRuntime()) {
    const result = await invokeHostLauncherState('games.list');
    return result.value?.games ?? fallbackGames;
  }
  return fallbackGames;
}

export async function joinGame(gameId: string): Promise<CommandResult<LauncherState>> {
  return isHostRuntime()
    ? invokeHostLauncherState('games.join', { gameId })
    : commandUnavailable('games.join', 'Joining a game requires the Fun host runtime.');
}

export async function listAuthorizedProjects(): Promise<AuthorizedProject[]> {
  if (isHostRuntime()) {
    const result = await invokeHostLauncherState('projects.authorized.list');
    return result.value?.projects ?? fallbackProjects;
  }
  return fallbackProjects;
}

export async function openProjectEditor(projectId: string): Promise<CommandResult<LauncherState>> {
  return isHostRuntime()
    ? invokeHostLauncherState('project.edit.open', { projectId })
    : { ok: true, value: setBrowserFallbackLauncherMode('editor', projectId), diagnostics: [] };
}

export async function activateEditor(): Promise<CommandResult<LauncherState>> {
  return isHostRuntime()
    ? invokeHostLauncherState('editor.activate')
    : { ok: true, value: setBrowserFallbackLauncherMode('editor'), diagnostics: [] };
}

export async function deactivateEditor(): Promise<CommandResult<LauncherState>> {
  return isHostRuntime()
    ? invokeHostLauncherState('editor.deactivate')
    : { ok: true, value: setBrowserFallbackLauncherMode('hidden'), diagnostics: [] };
}

export async function toggleEditorOverlay(): Promise<CommandResult<LauncherState>> {
  return isHostRuntime()
    ? invokeHostLauncherState('editor.overlay.toggle')
    : { ok: true, value: setBrowserFallbackLauncherMode(browserFallbackLauncherState.launcher_mode === 'editor' ? 'hidden' : 'editor'), diagnostics: [] };
}

export async function getRuntimeHostStatus(): Promise<RuntimeHostStatus> {
  if (isHostRuntime()) {
    const result = await invokeHostLauncherState('runtime.host.status');
    return result.value?.runtime_host_status ?? fallbackRuntimeHostStatus;
  }
  return fallbackRuntimeHostStatus;
}

export async function launchRetiredEngineDemo(): Promise<CommandResult<HostedInstanceSummary>> {
  return isHostRuntime()
    ? invokeHost<CommandResult<HostedInstanceSummary>>('retired_engine.demo.launch')
    : commandUnavailable('retired_engine.demo.launch', 'The RetiredEngine demo can only be launched from the Fun host.');
}

export async function openProject(path?: string): Promise<CommandResult<ProjectSummary>> {
  return isHostRuntime()
    ? invokeHost<CommandResult<ProjectSummary>>('project.open', { path: path || null })
    : { ok: true, value: fallbackProject, diagnostics: fallbackProject.diagnostics };
}

export async function getCurrentProject(): Promise<ProjectSummary | null> {
  return isHostRuntime() ? invokeHost<ProjectSummary | null>('project.current.get') : fallbackProject;
}

export async function listRecentProjects(): Promise<ProjectReference[]> {
  if (isHostRuntime()) {
    return invokeHost<ProjectReference[]>('projects.recent.list');
  }

  return [
    {
      id: fallbackProject.id,
      root_path: fallbackProject.root_path,
      display_name: fallbackProject.display_name
    }
  ];
}

export async function listMaterialShaders(projectId: string): Promise<CommandResult<MaterialShaderCatalog>> {
  return isHostRuntime()
    ? invokeHost<CommandResult<MaterialShaderCatalog>>('material.shader.list', { projectId })
    : commandUnavailable('material.shader.list', 'Native WGSL shader listing requires the Fun host.');
}

export async function loadMaterialShader(
  request: MaterialShaderLoadRequest
): Promise<CommandResult<MaterialShaderDocument>> {
  return isHostRuntime()
    ? invokeHost<CommandResult<MaterialShaderDocument>>('material.shader.load', { request })
    : commandUnavailable('material.shader.load', 'Native WGSL shader loading requires the Fun host.');
}

export async function saveMaterialShader(
  request: MaterialShaderSaveRequest
): Promise<CommandResult<MaterialShaderDocument>> {
  return isHostRuntime()
    ? invokeHost<CommandResult<MaterialShaderDocument>>('material.shader.save', { request })
    : commandUnavailable('material.shader.save', 'Native WGSL shader saving requires the Fun host.');
}

export async function listProjectEntities(filter?: EntityFilter): Promise<EntityRowSummary[]> {
  const stream = await openEntityStream({
    filter: filter ?? null,
    page_size: DEFAULT_ENTITY_PAGE_SIZE,
    overscan_rows: DEFAULT_ENTITY_OVERSCAN,
    synthetic_count: null
  });

  if (!stream.ok || !stream.value) {
    return [];
  }

  const page = await pageEntityStream({
    cursor: stream.value.cursor,
    offset: 0,
    limit: DEFAULT_ENTITY_PAGE_SIZE
  });
  await closeEntityStream(stream.value.cursor);
  return page.value?.rows ?? [];
}

export async function getProjectBsnIndex(): Promise<CommandResult<BsnIndexSummary>> {
  return isHostRuntime()
    ? invokeHost<CommandResult<BsnIndexSummary>>('project.bsn.index')
    : { ok: true, value: fallbackBsnIndex, diagnostics: [] };
}

export async function openEntityStream(
  request: EntityStreamOpenRequest
): Promise<CommandResult<EntityStreamCursor>> {
  if (isHostRuntime()) {
    return invokeHost<CommandResult<EntityStreamCursor>>('entity_stream.open', { request });
  }

  fallbackActiveFilter = request.filter;
  fallbackActiveSyntheticCount = request.synthetic_count ?? MOCK_ENTITY_COUNT;
  const diagnostics =
    fallbackActiveSyntheticCount > 0
      ? [
          diagnostic(
            'entity_stream.mock_data_active',
            'warning',
            `Browser preview mock data is active; ${fallbackActiveSyntheticCount} synthetic rows are visible.`
          )
        ]
      : [];
  return {
    ok: true,
    value: {
      cursor: 'browser-preview-cursor',
      revision: fallbackBsnIndex.revision,
      total_rows: fallbackRowsForFilter(request.filter, fallbackActiveSyntheticCount).length,
      page_size: request.page_size ?? DEFAULT_ENTITY_PAGE_SIZE,
      overscan_rows: request.overscan_rows ?? DEFAULT_ENTITY_OVERSCAN
    },
    diagnostics
  };
}

export async function openLiveEntityStream(
  request: LiveEntityStreamOpenRequest
): Promise<CommandResult<EntityStreamCursor>> {
  return isHostRuntime()
    ? invokeHost<CommandResult<EntityStreamCursor>>('live_entity_stream.open', { request })
    : commandUnavailable('live_entity_stream.open', 'Live runtime rows require an authenticated Fun host inspector connection.');
}

export async function pageEntityStream(
  request: EntityStreamPageRequest
): Promise<CommandResult<EntityStreamPage>> {
  if (isHostRuntime()) {
    return invokeHost<CommandResult<EntityStreamPage>>('entity_stream.page', { request });
  }

  const limit = request.limit ?? DEFAULT_ENTITY_PAGE_SIZE;
  const rows = fallbackRowsForFilter(fallbackActiveFilter, fallbackActiveSyntheticCount);
  return {
    ok: true,
    value: {
      cursor: request.cursor,
      revision: fallbackBsnIndex.revision,
      offset: request.offset,
      limit,
      total_rows: rows.length,
      rows: rows.slice(request.offset, request.offset + limit)
    },
    diagnostics: []
  };
}

export async function closeEntityStream(cursor: string): Promise<CommandResult<EntityStreamCloseSummary>> {
  return isHostRuntime()
    ? invokeHost<CommandResult<EntityStreamCloseSummary>>('entity_stream.close', { cursor })
    : { ok: true, value: { cursor, closed: true }, diagnostics: [] };
}

export async function getEntityDetails(entityId: string): Promise<CommandResult<EntityDetails>> {
  if (isHostRuntime()) {
    return invokeHost<CommandResult<EntityDetails>>('entity.details.get', { entityId });
  }

  const row = fallbackRowById(entityId) ?? fallbackRows(0, 1)[0];
  return {
    ok: true,
    value: {
      row,
      components: fallbackComponents(row),
      resources: [
        componentSummary('SourceKind', row.source_kind, row.source)
      ],
      source_preview: fallbackSourcePreview(row)
    },
    diagnostics: []
  };
}

export async function patchEntityTransform(
  request: TransformPatchRequest
): Promise<CommandResult<TransformPatchResult>> {
  return isHostRuntime()
    ? invokeHost<CommandResult<TransformPatchResult>>('entity.transform.patch', { request })
    : commandUnavailable('entity.transform.patch', 'Server Transform mutations require an authenticated Fun host inspector connection.');
}

export async function pickProjectDirectory(): Promise<CommandResult<ProjectReference>> {
  return isHostRuntime()
    ? invokeHost<CommandResult<ProjectReference>>('project.picker.open')
    : commandUnavailable('project.picker.open', 'The native project picker is only available from the Fun host.');
}

export async function launchClientViewport(
  projectId: string,
  buildProfile: BuildProfile
): Promise<CommandResult<ViewportInstanceSummary>> {
  return isHostRuntime()
    ? invokeHostViewportState('viewport.client.launch', { projectId, buildProfile }, projectId)
    : commandUnavailable('viewport.client.launch', 'Switching to the current Fun client requires the Fun host.');
}

export async function stopClientViewport(
  instanceId: string,
  sceneId: string | null
): Promise<CommandResult<ViewportInstanceSummary>> {
  return isHostRuntime()
    ? invokeHostViewportState('viewport.client.stop', { instanceId, sceneId })
    : commandUnavailable('viewport.client.stop', 'The current Fun client is only controlled from the Fun host.');
}

export async function ensurePreviewViewport(
  projectId: string,
  sceneId: string | null
): Promise<CommandResult<ViewportInstanceSummary>> {
  return isHostRuntime()
    ? invokeHostViewportState('preview.viewport.ensure', { projectId, sceneId }, projectId)
    : commandUnavailable('preview.viewport.ensure', 'Current-client preview requires the Fun host.');
}

export async function stopPreviewViewport(
  instanceId: string
): Promise<CommandResult<ViewportInstanceSummary>> {
  return isHostRuntime()
    ? invokeHostViewportState('preview.viewport.stop', { instanceId })
    : commandUnavailable('preview.viewport.stop', 'Current-client preview requires the Fun host.');
}

export async function ensureEditorPreview(
  projectId: string,
  sceneId: string | null,
  rect?: ViewportRect | null
): Promise<CommandResult<PreviewRendererStatus>> {
  return isHostRuntime()
    ? invokeHostPreviewState('preview.renderer.ensure', { projectId, sceneId, rect: rect ?? null })
    : { ok: true, value: fallbackPreviewStatus(sceneId, rect), diagnostics: [] };
}

export async function resizeEditorPreview(
  projectId: string,
  sceneId: string | null,
  rect: ViewportRect
): Promise<CommandResult<PreviewRendererStatus>> {
  return isHostRuntime()
    ? invokeHostPreviewState('preview.renderer.resize', { projectId, sceneId, rect })
    : { ok: true, value: fallbackPreviewStatus(sceneId, rect), diagnostics: [] };
}

export async function getEditorPreviewFrame(): Promise<CommandResult<PreviewRendererStatus>> {
  return isHostRuntime()
    ? invokeHostPreviewState('preview.renderer.frame.get')
    : { ok: true, value: fallbackPreviewStatus(null, null), diagnostics: [] };
}

export async function setEditorPreviewScene(
  projectId: string,
  sceneId: string
): Promise<CommandResult<PreviewRendererStatus>> {
  return isHostRuntime()
    ? invokeHostPreviewState('preview.renderer.scene.set', { projectId, sceneId })
    : { ok: true, value: fallbackPreviewStatus(sceneId, null), diagnostics: [] };
}

export async function getEditorPreviewStatus(): Promise<CommandResult<PreviewRendererStatus>> {
  return isHostRuntime()
    ? invokeHostPreviewState('preview.renderer.status.get')
    : { ok: true, value: fallbackPreviewStatus(null, null), diagnostics: [] };
}

export async function focusClientViewport(
  instanceId: string
): Promise<CommandResult<ViewportInstanceSummary>> {
  return isHostRuntime()
    ? invokeHostViewportState('viewport.client.focus', { instanceId })
    : commandUnavailable('viewport.client.focus', 'Returning input to gameplay requires the Fun host.');
}

export async function resizeClientViewport(
  instanceId: string,
  rect: ViewportRect
): Promise<CommandResult<ViewportInstanceSummary>> {
  return isHostRuntime()
    ? invokeHostViewportState('viewport.client.resize', { instanceId, rect })
    : commandUnavailable('viewport.client.resize', 'Updating the current client layout requires the Fun host.');
}

export async function returnToGame(): Promise<CommandResult<ViewportInstanceSummary>> {
  return isHostRuntime()
    ? invokeHostViewportState('runtime.input.set_owner', { owner: 'gameplay' })
    : commandUnavailable('runtime.input.set_owner', 'Runtime input ownership can only be changed from the Fun host.');
}

export async function setHostInputOwner(owner: FunInputOwner): Promise<CommandResult<ViewportInstanceSummary>> {
  return isHostRuntime()
    ? invokeHostViewportState('runtime.input.set_owner', { owner })
    : { ok: true, value: null, diagnostics: [] };
}

export async function executeHostCommandbarAction(
  action: CommandbarAction
): Promise<CommandResult<unknown>> {
  const request = commandbarHostExecutionPayload(action);
  if (!request) {
    return { ok: true, value: null, diagnostics: [] };
  }
  if (isHostRuntime()) {
    return invokeHost<CommandResult<unknown>>('host.commandbar.execute', request);
  }
  if (action.kind === 'filter_diagnostics' || action.kind === 'select_entity' || action.kind === 'set_context') {
    return { ok: true, value: null, diagnostics: [] };
  }
  return commandUnavailable('host.commandbar.execute', 'Commandbar execution requires the Fun host.');
}

export async function launchServerRuntime(
  projectId: string
): Promise<CommandResult<RuntimeInstanceSummary>> {
  return isHostRuntime()
    ? invokeHost<CommandResult<RuntimeInstanceSummary>>('runtime.server.launch', { projectId })
    : commandUnavailable('runtime.server.launch', 'The Fun server runtime can only be launched from the Fun host.');
}

function commandbarHostExecutionPayload(
  action: CommandbarAction
): { command_id: string; payload: Record<string, unknown> | null } | null {
  if (action.kind === 'noop') {
    return null;
  }
  if (action.kind === 'command') {
    return { command_id: action.commandId, payload: null };
  }
  if (action.kind === 'open_project') {
    return { command_id: 'project.open', payload: { path: action.path } };
  }
  if (action.kind === 'set_context') {
    return { command_id: `editor.set_context.${action.contextId}`, payload: { context_id: action.contextId } };
  }
  if (action.kind === 'filter_diagnostics') {
    return { command_id: 'runtime.diagnostics.list', payload: { severity: action.severity ?? null, tag: action.tag ?? null } };
  }
  if (action.kind === 'select_entity') {
    return { command_id: 'entity.details.get', payload: { entity_id: action.entityId } };
  }
  if (action.kind === 'tool_call_preview') {
    return {
      command_id: action.toolCall.toolName,
      payload: {
        tool_call_id: action.toolCall.id,
        risk: action.toolCall.risk,
        requires_confirmation: action.toolCall.requiresConfirmation,
        arguments: action.toolCall.arguments
      }
    };
  }
  return null;
}

export async function getRuntimeStatus(): Promise<RuntimeStatusSummary> {
  return isHostRuntime() ? invokeHost<RuntimeStatusSummary>('runtime.status.get') : fallbackRuntimeStatus;
}

export async function getRuntimeDiagnostics(
  target?: RuntimeDiagnosticsSnapshot['target']
): Promise<CommandResult<RuntimeDiagnosticsSnapshot>> {
  return isHostRuntime()
    ? invokeHost<CommandResult<RuntimeDiagnosticsSnapshot>>('runtime.diagnostics.list', { target: target ?? null })
    : commandUnavailable('runtime.diagnostics.list', 'Runtime diagnostics require the Fun host.');
}

export async function attachRuntimeInspector(
  request: RuntimeAttachRequest
): Promise<CommandResult<RuntimeStatusSummary>> {
  return isHostRuntime()
    ? invokeHost<CommandResult<RuntimeStatusSummary>>('runtime.inspector.attach', { request })
    : commandUnavailable('runtime.inspector.attach', 'Manual runtime attach requires the Fun host.');
}

export async function getAuthSession(): Promise<AuthSessionSummary> {
  return isHostRuntime() ? invokeHost<AuthSessionSummary>('auth.session.get') : fallbackAuth;
}

export async function requestBackendAuthTicket(
  request: BackendAuthTicketRequest
): Promise<CommandResult<BackendAuthTicket>> {
  if (isHostRuntime()) {
    return invokeHost<CommandResult<BackendAuthTicket>>('auth.ticket.request', request);
  }

  return commandUnavailable('auth.ticket.request', 'Backend auth tickets require the Fun host.');
}

export async function requestBackendAccountTicket(
  request: BackendAccountTicketLoginRequest
): Promise<CommandResult<BackendAuthTicket>> {
  if (isHostRuntime()) {
    const command = request.mode === 'register' ? 'account.register' : 'account.login';
    return invokeHost<CommandResult<BackendAuthTicket>>(command, request);
  }

  return commandUnavailable('account.login', 'Backend account tickets require the Fun host.');
}

export async function getBackendAuthSession(): Promise<BackendAuthSessionState> {
  if (isHostRuntime()) {
    return invokeHost<BackendAuthSessionState>('backend.auth.session.get');
  }
  return {
    authenticated: false,
    backend_base_url: 'http://127.0.0.1:8787',
    profile: null,
    ticket: null
  };
}

export async function logoutBackendAccount(): Promise<CommandResult<BackendAuthSessionState>> {
  if (isHostRuntime()) {
    return invokeHost<CommandResult<BackendAuthSessionState>>('account.logout');
  }
  return { ok: true, value: await getBackendAuthSession(), diagnostics: [] };
}

function fallbackRows(offset: number, limit: number): EntityRowSummary[] {
  return fallbackRowsForFilter(null, fallbackActiveSyntheticCount).slice(offset, offset + limit);
}

function fallbackRowsForFilter(filter: EntityFilter | null, syntheticCount: number): EntityRowSummary[] {
  const projectRows = fallbackProjectRows();
  const syntheticRows = Array.from({ length: syntheticCount }, (_, index) => syntheticRow(index));
  return [...projectRows, ...syntheticRows].filter((row) => matchesFallbackFilter(row, filter));
}

function fallbackProjectRows(): EntityRowSummary[] {
  const bsnRows = fallbackBsnIndex.records.flatMap((record) =>
    record.detected_names.map((name) => ({
      id: `${record.id}::${name}`,
      name,
      domain: record.domain ?? 'shared',
      source_kind: 'source_bsn' as EntitySourceKind,
      source: record.file_path,
      source_span: record.span,
      component_count: record.component_type_tokens.length,
      selectable: true,
      live_snapshot: false,
      revision: record.span.start.line
    }))
  );

  const crateRows = fallbackProject.crates
    .filter((projectCrate): projectCrate is ProjectCrate & { domain: EcsDomain } => projectCrate.domain !== null)
    .map((projectCrate) => ({
      id: `${fallbackProject.id}::crate::${projectCrate.name}`,
      name: `${projectCrate.name} domain root`,
      domain: projectCrate.domain,
      source_kind: 'shared_schema' as EntitySourceKind,
      source: projectCrate.manifest_path,
      source_span: null,
      component_count: projectCrate.target_names.length,
      selectable: true,
      live_snapshot: false,
      revision: fallbackBsnIndex.revision
    }));

  return [...bsnRows, ...crateRows];
}

function syntheticRow(index: number): EntityRowSummary {
  const domain = domainForIndex(index);
  return {
    id: `${fallbackProject.id}::synthetic::${index.toString().padStart(5, '0')}`,
    name: `SyntheticEntity${index.toString().padStart(5, '0')}`,
    domain,
    source_kind: sourceKindForDomain(domain),
    source: 'synthetic://entity-stream',
    source_span: null,
    component_count: 6,
    selectable: true,
    live_snapshot: true,
    revision: fallbackBsnIndex.revision
  };
}

function fallbackRowById(entityId: string): EntityRowSummary | null {
  return fallbackRowsForFilter(null, fallbackActiveSyntheticCount).find((row) => row.id === entityId) ?? null;
}

function fallbackComponents(row: EntityRowSummary): ComponentSummary[] {
  if (row.source_kind === 'source_bsn') {
    return fallbackBsnIndex.records[0].component_type_tokens.map((name) =>
      componentSummary(name, 'from Fun scene token scan', row.source)
    );
  }

  if (row.source.startsWith('synthetic://')) {
    return Array.from({ length: row.component_count }, (_, index) =>
      componentSummary(
        `SyntheticComponent${index.toString().padStart(2, '0')}`,
        `mock revision ${row.revision}`,
        row.source
      )
    );
  }

  return [];
}

function componentSummary(name: string, valuePreview: string | null, source: string | null): ComponentSummary {
  return {
    name,
    value_preview: valuePreview,
    source,
    component_kind: null,
    schema_label: null,
    mutability: null,
    replication_policy: null,
    ui_group: null,
    ui_widget: null,
    importance: null,
    raw_payload_bytes: 0,
    read_only: true
  };
}

function matchesFallbackFilter(row: EntityRowSummary, filter: EntityFilter | null): boolean {
  if (!filter) {
    return true;
  }
  if (filter.domain && filter.domain !== row.domain) {
    return false;
  }
  if (filter.source === 'bsn' && row.source_kind !== 'source_bsn') {
    return false;
  }
  if (
    filter.source === 'live' &&
    row.source_kind !== 'client_live' &&
    row.source_kind !== 'server_live'
  ) {
    return false;
  }
  const query = filter.query?.trim().toLowerCase();
  if (!query) {
    return true;
  }
  return [row.id, row.name, row.source, row.domain, row.source_kind].join(' ').toLowerCase().includes(query);
}

function fallbackSourcePreview(row: EntityRowSummary): SourcePreview | null {
  if (row.source_kind !== 'source_bsn') {
    return null;
  }

  return {
    path: row.source,
    start_line: row.source_span?.start.line ?? 1,
    end_line: row.source_span?.end.line ?? 1,
    text: 'commands.spawn_fun_scene_list(fun_list![...]);'
  };
}

function commandUnavailable<T>(commandId: string, message: string): CommandResult<T> {
  return {
    ok: false,
    value: null,
    diagnostics: [
      {
        code: 'host.runtime_missing',
        level: 'warning',
        message,
        target_path: targetPath,
        hosted_instance_id: commandId
      }
    ]
  };
}

function crate(name: string, role: ProjectCrate['role'], domain: ProjectCrate['domain']): ProjectCrate {
  return {
    name,
    manifest_path: `${targetPath}\\${name}\\Cargo.toml`,
    target_names: [name],
    role,
    domain
  };
}

function descriptor(
  id: string,
  title: string,
  summary: string,
  category: string,
  input_description: string,
  output_description: string
): HostCommandDescriptor {
  return {
    id,
    title,
    summary,
    category,
    input_description,
    output_description,
    payload_schema_id: `browser-preview.${id}`,
    required_capability: 'ReadLauncher',
    risk: {
      level: 'read',
      mutates_state: false,
      requires_confirmation: false,
      filesystem_access: false,
      network_access: false,
      tool_stub: false
    },
    rust_owner: 'browser_preview'
  };
}

function diagnostic(code: string, level: Diagnostic['level'], message: string): Diagnostic {
  return {
    code,
    level,
    message,
    target_path: targetPath,
    hosted_instance_id: null
  };
}

function status(state: string, detail: string, authenticated: boolean): RuntimeStatusSummary['client_process'] {
  return {
    state,
    detail,
    authenticated,
    last_error: null,
    protocol_version: null,
    schema_revision: null,
    diagnostic_schema_revision: null,
    last_heartbeat_unix_ms: null,
    subscriptions: [],
    backpressure: 'none',
    reconnect_state: authenticated ? 'connected' : 'idle'
  };
}

function fallbackPreviewStatus(sceneId: string | null, rect?: ViewportRect | null): PreviewRendererStatus {
  const width = Math.max(1, Math.round((rect?.width ?? 1280) * (rect?.scale_factor ?? 1)));
  const height = Math.max(1, Math.round((rect?.height ?? 720) * (rect?.scale_factor ?? 1)));
  return {
    status: 'ready',
    scene: {
      scene_id: sceneId ?? fallbackBsnIndex.records[0]?.id ?? 'fallback-default-scene',
      entity_count: 7,
      revision: fallbackBsnIndex.revision,
      manifest_signature: 1
    },
    frame: {
      frame_index: Date.now(),
      rgba_byte_len: 0,
      readback_status: 'current_client_background'
    },
    target: {
      width,
      height,
      format: 'CurrentClient',
      texture_usages: 'NATIVE_UI_OVERLAY_ONLY'
    },
    signature_warning: null
  };
}

function domainForIndex(index: number): EcsDomain {
  const domains: EcsDomain[] = ['client', 'server', 'shared'];
  return domains[index % domains.length];
}

function sourceKindForDomain(domain: EcsDomain): EntitySourceKind {
  if (domain === 'client') {
    return 'client_live';
  }
  if (domain === 'server') {
    return 'server_live';
  }
  return 'shared_schema';
}

export function isHostRuntime(): boolean {
  return hasHostCommandTransport();
}
