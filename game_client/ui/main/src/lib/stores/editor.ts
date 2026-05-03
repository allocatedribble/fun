import { get, writable } from 'svelte/store';
import {
  DEFAULT_ENTITY_OVERSCAN,
  DEFAULT_ENTITY_PAGE_SIZE,
  MOCK_ENTITY_COUNT,
  activateEditor,
  browserFallbackLauncherState,
  closeEntityStream,
  deactivateEditor,
  executeHostCommandbarAction,
  ensureEditorPreview,
  fallbackCommands,
  fallbackEvents,
  fallbackProject,
  fallbackRuntimeHostStatus,
  fallbackRuntimeStatus,
  fallbackStatus,
  focusClientViewport,
  getEditorPreviewFrame,
  getCurrentProject,
  getEditorStatus,
  getEntityDetails,
  getLauncherState,
  getRuntimeDiagnostics,
  getRuntimeHostStatus,
  getRuntimeStatus,
  hideLauncher,
  isHostRuntime,
  joinGame,
  launchClientViewport,
  launchServerRuntime,
  listAuthorizedProjects,
  listEditorCommands,
  listEditorEvents,
  listGames,
  listRecentProjects,
  logoutBackendAccount,
  openEntityStream,
  openLiveEntityStream,
  openProjectEditor,
  openProject,
  pageEntityStream,
  patchEntityTransform,
  pickProjectDirectory,
  resizeEditorPreview,
  resizeClientViewport,
  requestBackendAccountTicket,
  requestBackendAuthTicket,
  setEditorPreviewScene,
  setHostInputOwner,
  showLauncher,
  stopClientViewport,
  toggleEditorOverlay
} from '../commands';
import type { FunHostSnapshotPayload, FunHostState } from '../commands';
import { subscribeHost } from '../host/commands';
import { hostStateFromPayload, hostStore, type HostStatePatchPayload, type HostStoreState } from './host';
import { intentSummary, inferCommandbarMode, routeCommandbarIntent } from '../commandbar/intentRouter';
import { buildCommandbarResults } from '../commandbar/searchIndex';
import { defaultExperienceScope } from '../commandbar/scopeRegistry';
import { toolCallPreview } from '../commandbar/toolRegistry';
import type {
  BuildProfile,
  CommandbarAction,
  CommandResult,
  Diagnostic,
  DiagnosticSeverity,
  DiagnosticsSortMode,
  DomainFilter,
  EditorActivationSummary,
  EditorContextId,
  EditorUiState,
  EntityFilter,
  EntityRowSummary,
  ExperienceScope,
  LauncherMode,
  OverviewSelection,
  PreviewLifecycleStatus,
  PreviewRendererStatus,
  LauncherState,
  RenderMode,
  RuntimeDiagnosticsSnapshot,
  RuntimeHostStatus,
  RuntimeStatusSummary,
  SourceFilter,
  TransformPatchRequest,
  ViewportInstanceSummary,
  ViewportRect
} from '../types';

const runtimeRefreshMinMs = 900;
const searchDebounceMs = 150;
const diagnosticsCap = 240;
const commandbarResultCap = 64;
const diagnosticsPreferencePrefix = 'fun-editor:diagnostics-visible';

type RuntimePatchPayload =
  | RuntimeHostStatus
  | {
      runtimeHostStatus?: RuntimeHostStatus | null;
      runtime_host_status?: RuntimeHostStatus | null;
      runtimeStatus?: RuntimeStatusSummary | null;
      runtime_status?: RuntimeStatusSummary | null;
      diagnostics?: Diagnostic[];
    };

type DiagnosticsPatchPayload =
  | RuntimeDiagnosticsSnapshot
  | CommandResult<RuntimeDiagnosticsSnapshot>
  | {
      value?: RuntimeDiagnosticsSnapshot | null;
      diagnostics?: Diagnostic[];
    };

const emptyDiagnosticsFilters = {
  severities: [],
  tags: [],
  sources: [],
  scopes: [],
  currentFileOnly: false
} satisfies EditorUiState['diagnosticsView']['activeFilters'];

const initialState: EditorUiState = {
  launcherMode: 'launcher',
  hostMode: null,
  hostInputOwner: null,
  games: [],
  projects: [],
  selectedGameId: null,
  selectedProjectId: null,
  activeRuntime: null,
  authorization: {
    can_join_game: false,
    can_host_local: false,
    can_open_project: false,
    can_edit_project: false,
    can_resume_editor: false,
    can_stop_runtime: false,
    capabilities: [],
    reason: null
  },
  runtimeHostStatus: null,
  status: null,
  runtimeStatus: null,
  commands: [],
  events: [],
  project: null,
  recentProjects: [],
  viewport: null,
  viewportRect: null,
  editorContexts: [
    { id: 'overview', label: 'Overview', status: 'ready' },
    { id: 'preview', label: 'Preview', status: 'idle' },
    { id: 'live_client', label: 'Current Client', status: 'stopped' },
    { id: 'server', label: 'Server', status: 'stopped' },
    { id: 'graph', label: 'Graph', status: 'ready' },
    { id: 'diagnostics', label: 'Log', status: 'ready' }
  ],
  activeEditorContextId: 'overview',
  editorContext: 'overview',
  availableContexts: [
    { id: 'overview', label: 'Overview', status: 'ready' },
    { id: 'preview', label: 'Preview', status: 'idle' },
    { id: 'live_client', label: 'Current Client', status: 'stopped' },
    { id: 'server', label: 'Server', status: 'stopped' },
    { id: 'graph', label: 'Graph', status: 'ready' },
    { id: 'diagnostics', label: 'Log', status: 'ready' }
  ],
  contextSwitching: false,
  previewStatus: 'idle',
  previewError: null,
  previewFrameRevision: 0,
  previewSceneId: null,
  previewRenderStatus: {
    status: 'idle',
    sceneId: null,
    frameIndex: 0,
    warning: null,
    error: null,
    renderer: null
  },
  liveClientStatus: null,
  serverRuntime: null,
  overview: {
    query: '',
    selectedItem: null
  },
  entities: {
    cursor: null,
    rowsByIndex: {},
    totalRows: 0,
    pageSize: DEFAULT_ENTITY_PAGE_SIZE,
    overscanRows: DEFAULT_ENTITY_OVERSCAN,
    loading: false,
    query: '',
    domain: 'all',
    source: 'both',
    selectedId: null,
    selectedDetails: null
  },
  diagnostics: [],
  runtimeDiagnostics: null,
  diagnosticsView: {
    visible: false,
    height: 180,
    unreadCriticalCount: 0,
    unreadWarningCount: 0,
    unreadInfoCount: 0,
    activeFilters: emptyDiagnosticsFilters,
    filterMode: 'any',
    sortMode: 'newest',
    compact: true
  },
  account: {
    loginOpen: false,
    loading: false,
    backendUrl: 'http://127.0.0.1:8787',
    profile: null,
    ticket: null,
    error: null
  },
  commandbar: {
    focused: false,
    input: '',
    mode: 'identity',
    activeScope: { type: 'all_projects' },
    results: [],
    modelRoute: 'none'
  },
  projectPath: 'C:\\Users\\premi\\work\\project-FUN\\fun',
  activeSceneId: null,
  commandSearch: '',
  buildProfile: 'debug',
  renderMode: 'solari',
  bottomTab: 'diagnostics',
  lastCommandResult: null,
  loading: false,
  opening: false,
  launchingClient: false,
  launchingServer: false,
  stoppingClient: false,
  error: null
};

function createEditorStore() {
  const store = writable<EditorUiState>(initialState);
  let loadedPages = new Set<number>();
  let searchTimer: ReturnType<typeof setTimeout> | null = null;
  let lastRuntimeRefresh = 0;
  let activationUnlisten: (() => void) | null = null;
  let previewRequestToken = 0;

  function rememberCommandResult(result: CommandResult<unknown>): void {
    store.update((state) => ({ ...state, lastCommandResult: result }));
  }

  async function initialize(): Promise<void> {
    store.update((state) => ({ ...state, loading: true, error: null }));
    subscribeToHostEvents();

    try {
      if (isHostRuntime()) {
        await hostStore.initialize();
        return;
      }

      const [
        status,
        commands,
        events,
        project,
        recentProjects,
        runtimeStatus,
        runtimeDiagnostics,
        launcherState,
        runtimeHostStatus
      ] = await Promise.all([
        getEditorStatus(),
        listEditorCommands(),
        listEditorEvents(),
        getCurrentProject(),
        listRecentProjects(),
        getRuntimeStatus(),
        getRuntimeDiagnostics(),
        getLauncherState(),
        getRuntimeHostStatus()
      ]);

      store.update((state) => {
        const viewport = project?.current_viewport_status ?? null;
        const previewStatus = state.previewRenderStatus;
        const liveClientStatus = viewport?.kind === 'client' ? viewport : state.liveClientStatus;
        const diagnostics = appendDiagnostics(project?.diagnostics ?? [], runtimeDiagnostics.diagnostics);
        const diagnosticsView = diagnosticsViewForProject(state.diagnosticsView, project?.id ?? project?.root_path ?? null, diagnostics);
        const commandbar = commandbarForState(
          {
            ...state,
            project,
            recentProjects,
            viewport,
            liveClientStatus,
            serverRuntime: state.serverRuntime,
            diagnostics,
            runtimeDiagnostics: runtimeDiagnostics.value,
            commands
          },
          state.commandbar.input
        );
        return {
        ...state,
        ...launcherStateToStore(launcherState),
        runtimeHostStatus,
        status,
        commands,
        events,
        project,
        recentProjects,
        runtimeStatus,
        runtimeDiagnostics: runtimeDiagnostics.value,
        viewport,
        previewRenderStatus: previewStatus,
        ...previewLifecycleFields(previewStatus),
        liveClientStatus,
        ...contextFields(
          previewStatus.status,
          liveClientStatus?.status ?? 'stopped',
          state.serverRuntime?.process_status ?? 'stopped'
        ),
        projectPath: project?.root_path ?? status.target_fps_path,
        activeSceneId: project?.bsn_index.records[0]?.id ?? null,
        diagnostics,
        diagnosticsView,
        commandbar,
        loading: false
        };
      });

      if (project) {
        await reopenEntityStream();
        await ensureProjectPreview('project_selected');
      } else {
        await openDefaultProject();
      }
    } catch (error) {
      store.update((state) => ({ ...state, loading: false, error: errorMessage(error) }));
    }
  }

  function subscribeToHostEvents(): void {
    if (activationUnlisten) {
      return;
    }
    const unsubscribers = [
      subscribeHost<EditorActivationSummary>('host.editor.activation', (payload) => {
        store.update((state) => {
          const liveClientStatus = payload.viewport ?? state.liveClientStatus;
          return {
            ...state,
            viewport: payload.viewport ?? state.viewport,
            liveClientStatus,
            ...activeContextFields('live_client'),
            ...contextFields(
              state.previewRenderStatus.status,
              liveClientStatus?.status ?? 'running',
              state.serverRuntime?.process_status ?? 'stopped'
            )
          };
        });
        void refreshTrace();
      }),
      hostStore.subscribe(applyHostStoreState),
      subscribeHost<RuntimePatchPayload>('host.runtime.patch', applyRuntimePatch),
      subscribeHost<DiagnosticsPatchPayload>('host.diagnostics.patch', applyDiagnosticsPatch)
    ];
    activationUnlisten = () => {
      unsubscribers.forEach((unsubscribe) => unsubscribe());
      activationUnlisten = null;
    };
  }

  function applyHostStoreState(hostState: HostStoreState): void {
    if (!hostState.ready) {
      return;
    }
    if (hostState.snapshot) {
      applyHostSnapshot(hostState.snapshot);
    } else if (hostState.state) {
      applyHostSnapshot(hostState.state);
    }
  }

  function applyHostSnapshot(payload: HostStatePatchPayload): void {
    const hostState = hostStateFromPayload(payload);
    if (!hostState) {
      return;
    }
    const launcherMode = launcherModeForHostMode(hostState.mode);
    const runtimeHostStatus = {
      ...fallbackRuntimeHostStatus,
      launcher_mode: launcherMode,
      host_state: hostState.mode,
      active_runtime: null,
      runtime_status: fallbackRuntimeStatus
    } satisfies RuntimeHostStatus;
    const launcherState = {
      ...browserFallbackLauncherState,
      launcher_mode: launcherMode,
      selected_game_id: hostState.launcher.selected_game_id ?? browserFallbackLauncherState.selected_game_id,
      selected_project_id: hostState.launcher.selected_project_id ?? browserFallbackLauncherState.selected_project_id,
      runtime_host_status: runtimeHostStatus,
      authorization: {
        ...browserFallbackLauncherState.authorization,
        can_open_project: hostState.project.local_project_authorized,
        can_edit_project: hostState.project.local_project_authorized,
        can_resume_editor: hostState.runtime.return_to_game_available
      }
    } satisfies LauncherState;
    store.update((state) => {
      const project = state.project ?? fallbackProject;
      const liveClientStatus = currentClientViewportForHostState(hostState, project.id) ?? state.liveClientStatus;
      const recentProjects = state.recentProjects.length > 0 ? state.recentProjects : [
        {
          id: fallbackProject.id,
          root_path: fallbackProject.root_path,
          display_name: fallbackProject.display_name
        }
      ];
      const nextState = {
        ...state,
        ...launcherStateToStore(launcherState),
        hostMode: hostState.mode,
        hostInputOwner: hostState.input_owner,
        activeEditorContextId: hostRouteToContext(hostState.editor.active_route) ?? state.activeEditorContextId,
        editorContext: hostRouteToContext(hostState.editor.active_route) ?? state.editorContext,
        status: state.status ?? fallbackStatus,
        commands: state.commands.length > 0 ? state.commands : fallbackCommands,
        events: state.events.length > 0 ? state.events : fallbackEvents,
        project,
        viewport: liveClientStatus ?? state.viewport,
        liveClientStatus,
        recentProjects,
        runtimeStatus: state.runtimeStatus ?? fallbackRuntimeStatus,
        runtimeHostStatus,
        projectPath: hostState.project.active_project_root ?? project.root_path,
        loading: false,
        error: null
      };
      return {
        ...nextState,
        commandbar: commandbarForState(nextState, state.commandbar.input)
      };
    });
  }

  function hostRouteToContext(route: FunHostState['editor']['active_route']): EditorContextId | null {
    if (!route) {
      return null;
    }
    if (route === 'live_client') {
      return 'live_client';
    }
    return route;
  }

  function launcherModeForHostMode(mode: FunHostState['mode']): LauncherMode {
    if (mode === 'editor' || mode === 'editor_overlay') {
      return 'editor';
    }
    if (mode === 'launcher' || mode === 'loading' || mode === 'boot') {
      return 'launcher';
    }
    return 'hidden';
  }

  function currentClientViewportForHostState(
    hostState: FunHostState,
    projectId: string
  ): ViewportInstanceSummary | null {
    if (!hostState.game.render_visible_behind_ui && hostState.mode !== 'game') {
      return null;
    }
    return {
      id: hostState.runtime.active_runtime_id ?? 'current-client',
      project_id: hostState.project.active_project_id ?? projectId,
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

  function applyRuntimePatch(payload: RuntimePatchPayload): void {
    if (!payload || typeof payload !== 'object') {
      return;
    }
    const record = payload as {
      runtimeHostStatus?: RuntimeHostStatus | null;
      runtime_host_status?: RuntimeHostStatus | null;
      runtimeStatus?: RuntimeStatusSummary | null;
      runtime_status?: RuntimeStatusSummary | null;
      diagnostics?: Diagnostic[];
    };
    const runtimeHostStatus =
      'host_state' in record
        ? (record as RuntimeHostStatus)
        : record.runtimeHostStatus ?? record.runtime_host_status ?? null;
    const runtimeStatus = record.runtimeStatus ?? record.runtime_status ?? runtimeHostStatus?.runtime_status ?? null;
    const diagnostics = Array.isArray(record.diagnostics) ? record.diagnostics : [];
    store.update((state) => {
      const mergedDiagnostics = appendDiagnostics(state.diagnostics, diagnostics);
      const nextState = {
        ...state,
        runtimeHostStatus: runtimeHostStatus ?? state.runtimeHostStatus,
        activeRuntime: runtimeHostStatus?.active_runtime ?? state.activeRuntime,
        runtimeStatus: runtimeStatus ?? state.runtimeStatus,
        diagnostics: mergedDiagnostics,
        diagnosticsView: diagnosticsViewWithCounts(state.diagnosticsView, mergedDiagnostics)
      };
      return {
        ...nextState,
        commandbar: commandbarForState(nextState, state.commandbar.input)
      };
    });
  }

  function applyDiagnosticsPatch(payload: DiagnosticsPatchPayload): void {
    if (!payload || typeof payload !== 'object') {
      return;
    }
    const record = payload as {
      value?: RuntimeDiagnosticsSnapshot | null;
      diagnostics?: Diagnostic[];
      records?: unknown[];
    };
    const diagnostics = Array.isArray(record.diagnostics) ? record.diagnostics : [];
    const value = record.value ?? (Array.isArray(record.records) ? (record as RuntimeDiagnosticsSnapshot) : null);
    store.update((state) => {
      const mergedDiagnostics = appendDiagnostics(state.diagnostics, diagnostics);
      const nextState = {
        ...state,
        runtimeDiagnostics: value ?? state.runtimeDiagnostics,
        diagnostics: mergedDiagnostics,
        diagnosticsView: diagnosticsViewWithCounts(state.diagnosticsView, mergedDiagnostics)
      };
      return {
        ...nextState,
        commandbar: commandbarForState(nextState, state.commandbar.input)
      };
    });
  }

  async function refreshTrace(): Promise<void> {
    if (isHostRuntime()) {
      await hostStore.refresh();
      return;
    }

    const [status, events, recentProjects, runtimeStatus, runtimeDiagnostics, launcherState, runtimeHostStatus] = await Promise.all([
      getEditorStatus(),
      listEditorEvents(),
      listRecentProjects(),
      getRuntimeStatus(),
      getRuntimeDiagnostics(),
      getLauncherState(),
      getRuntimeHostStatus()
    ]);

    store.update((state) => {
      const diagnostics = appendDiagnostics(state.diagnostics, runtimeDiagnostics.diagnostics);
      const nextRuntimeDiagnostics = runtimeDiagnostics.value ?? state.runtimeDiagnostics;
      const launcherFields = launcherStateToStore(launcherState);
      const nextState = {
        ...state,
        ...launcherFields,
        runtimeHostStatus,
        status,
        events,
        recentProjects,
        runtimeStatus,
        runtimeDiagnostics: nextRuntimeDiagnostics,
        diagnostics,
        diagnosticsView: diagnosticsViewWithCounts(state.diagnosticsView, diagnostics)
      };
      return {
        ...nextState,
        commandbar: commandbarForState(nextState, state.commandbar.input)
      };
    });
  }

  async function refreshRuntime(): Promise<void> {
    if (isHostRuntime()) {
      await hostStore.refresh();
      return;
    }

    const now = performance.now();
    if (now - lastRuntimeRefresh < runtimeRefreshMinMs) {
      return;
    }

    lastRuntimeRefresh = now;
    const [runtimeStatus, runtimeDiagnostics, runtimeHostStatus] = await Promise.all([
      getRuntimeStatus(),
      getRuntimeDiagnostics(),
      getRuntimeHostStatus()
    ]);
    store.update((state) => {
      const diagnostics = appendDiagnostics(state.diagnostics, runtimeDiagnostics.diagnostics);
      const nextState = {
        ...state,
        runtimeHostStatus,
        activeRuntime: runtimeHostStatus.active_runtime,
        runtimeStatus,
        runtimeDiagnostics: runtimeDiagnostics.value ?? state.runtimeDiagnostics,
        diagnostics,
        diagnosticsView: diagnosticsViewWithCounts(state.diagnosticsView, diagnostics)
      };
      return {
        ...nextState,
        commandbar: commandbarForState(nextState, state.commandbar.input)
      };
    });
  }

  async function openDefaultProject(): Promise<void> {
    const state = get(store);
    await openProjectAt(state.status?.target_fps_path ?? state.projectPath);
  }

  async function openTypedProject(): Promise<void> {
    await openProjectAt(get(store).projectPath);
  }

  async function openProjectAt(path?: string): Promise<void> {
    store.update((state) => ({ ...state, opening: true, error: null }));

    try {
      await closeCurrentEntityStream();
      const result = await openProject(path?.trim() || undefined);
      rememberCommandResult(result);
      const diagnostics = result.diagnostics;
      if (result.ok && result.value) {
        const project = result.value;
        store.update((state) => {
          const viewport = project.current_viewport_status;
          const previewStatus = {
            ...state.previewRenderStatus,
            sceneId: project.bsn_index.records[0]?.id ?? state.previewRenderStatus.sceneId
          };
          const liveClientStatus = viewport?.kind === 'client' ? viewport : state.liveClientStatus;
          const mergedDiagnostics = appendDiagnostics(project.diagnostics, diagnostics);
          const diagnosticsView = diagnosticsViewForProject(state.diagnosticsView, project.id, mergedDiagnostics);
          const nextState = {
          ...state,
          project,
          viewport,
          previewRenderStatus: previewStatus,
          ...previewLifecycleFields(previewStatus),
          liveClientStatus,
          ...activeContextFields('preview'),
          ...contextFields(
            previewStatus.status,
            liveClientStatus?.status ?? 'stopped',
            state.serverRuntime?.process_status ?? 'stopped'
          ),
          projectPath: project.root_path,
          activeSceneId: project.bsn_index.records[0]?.id ?? null,
          diagnostics: mergedDiagnostics,
          diagnosticsView,
          opening: false
          };
          return {
            ...nextState,
            commandbar: commandbarForState(nextState, state.commandbar.input)
          };
        });
        await reopenEntityStream();
        await ensureProjectPreview('project_opened');
      } else {
        store.update((state) => {
          const mergedDiagnostics = appendDiagnostics(state.diagnostics, diagnostics);
          const nextState = {
            ...state,
            diagnostics: mergedDiagnostics,
            diagnosticsView: diagnosticsViewWithCounts(state.diagnosticsView, mergedDiagnostics),
            opening: false
          };
          return {
            ...nextState,
            commandbar: commandbarForState(nextState, state.commandbar.input)
          };
        });
      }
      await refreshTrace();
    } catch (error) {
      store.update((state) => ({ ...state, opening: false, error: errorMessage(error) }));
    }
  }

  async function pickProject(): Promise<void> {
    store.update((state) => ({ ...state, opening: true, error: null }));
    const result = await pickProjectDirectory();
    rememberCommandResult(result);
    store.update((state) => {
      const diagnostics = appendDiagnostics(state.diagnostics, result.diagnostics);
      return {
        ...state,
        diagnostics,
        diagnosticsView: diagnosticsViewWithCounts(state.diagnosticsView, diagnostics)
      };
    });
    if (result.ok && result.value) {
      await openProjectAt(result.value.root_path);
    } else {
      store.update((state) => ({ ...state, opening: false }));
    }
  }

  async function reopenEntityStream(): Promise<void> {
    const state = get(store);
    if (!state.project) {
      return;
    }

    loadedPages = new Set<number>();
    store.update((current) => ({
      ...current,
      entities: {
        ...current.entities,
        loading: true,
        rowsByIndex: {},
        selectedId: null,
        selectedDetails: null
      }
    }));

    const filter = entityFilter(state.entities.domain, state.entities.source, state.entities.query);
    const result =
      state.entities.source === 'live'
        ? await openLiveEntityStream({
            target:
              state.entities.domain === 'client' || state.entities.domain === 'server'
                ? state.entities.domain
                : null,
            filter,
            page_size: DEFAULT_ENTITY_PAGE_SIZE,
            overscan_rows: DEFAULT_ENTITY_OVERSCAN
          })
        : await openEntityStream({
            filter,
            page_size: DEFAULT_ENTITY_PAGE_SIZE,
            overscan_rows: DEFAULT_ENTITY_OVERSCAN,
            synthetic_count: MOCK_ENTITY_COUNT > 0 ? MOCK_ENTITY_COUNT : null
          });
    rememberCommandResult(result);

    if (!result.ok || !result.value) {
      store.update((current) => ({
        ...current,
        diagnostics: appendDiagnostics(current.diagnostics, result.diagnostics),
        entities: { ...current.entities, loading: false }
      }));
      return;
    }

    const cursor = result.value;
    store.update((current) => ({
      ...current,
      diagnostics: appendDiagnostics(current.diagnostics, result.diagnostics),
      entities: {
        ...current.entities,
        cursor,
        totalRows: cursor.total_rows,
        pageSize: cursor.page_size,
        overscanRows: cursor.overscan_rows,
        loading: false
      }
    }));

    await ensureEntityRange(0, DEFAULT_ENTITY_PAGE_SIZE);
  }

  async function closeCurrentEntityStream(): Promise<void> {
    const cursor = get(store).entities.cursor?.cursor;
    if (cursor) {
      await closeEntityStream(cursor);
    }
  }

  async function ensureEntityRange(startIndex: number, endIndex: number): Promise<void> {
    const state = get(store);
    const cursor = state.entities.cursor;
    if (!cursor) {
      return;
    }

    const pageSize = state.entities.pageSize;
    const firstPage = Math.floor(Math.max(0, startIndex) / pageSize);
    const lastPage = Math.floor(Math.max(firstPage * pageSize, endIndex) / pageSize);
    const pages: number[] = [];
    for (let page = firstPage; page <= lastPage; page += 1) {
      if (!loadedPages.has(page)) {
        pages.push(page);
        loadedPages.add(page);
      }
    }

    if (pages.length === 0) {
      return;
    }

    store.update((current) => ({
      ...current,
      entities: { ...current.entities, loading: true }
    }));

    const pageResults = await Promise.all(
      pages.map((page) =>
        pageEntityStream({
          cursor: cursor.cursor,
          offset: page * pageSize,
          limit: pageSize
        })
      )
    );

    store.update((current) => {
      const rowsByIndex = { ...current.entities.rowsByIndex };
      const diagnostics: Diagnostic[] = [];
      for (const result of pageResults) {
        diagnostics.push(...result.diagnostics);
        const page = result.value;
        if (!page) {
          continue;
        }
        page.rows.forEach((row, index) => {
          rowsByIndex[page.offset + index] = row;
        });
      }

      return {
        ...current,
        diagnostics: appendDiagnostics(current.diagnostics, diagnostics),
        entities: { ...current.entities, rowsByIndex, loading: false }
      };
    });
  }

  async function selectEntity(row: EntityRowSummary): Promise<void> {
    store.update((state) => ({
      ...state,
      overview: { ...state.overview, selectedItem: overviewSelectionFromEntity(row) },
      entities: { ...state.entities, selectedId: row.id, selectedDetails: null }
    }));

    const result = await getEntityDetails(row.id);
    rememberCommandResult(result);
    store.update((state) => ({
      ...state,
      diagnostics: appendDiagnostics(state.diagnostics, result.diagnostics),
      entities: {
        ...state.entities,
        selectedDetails: result.value ?? null
      }
    }));
  }

  async function patchSelectedTransform(request: TransformPatchRequest): Promise<void> {
    const result = await patchEntityTransform(request);
    rememberCommandResult(result);
    store.update((state) => ({
      ...state,
      diagnostics: appendDiagnostics(state.diagnostics, result.diagnostics),
      bottomTab: 'diagnostics'
    }));

    const selected = get(store).entities.selectedId;
    if (result.ok && selected) {
      const details = await getEntityDetails(selected);
      rememberCommandResult(details);
      store.update((state) => ({
        ...state,
        diagnostics: appendDiagnostics(state.diagnostics, details.diagnostics),
        entities: {
          ...state.entities,
          selectedDetails: details.value ?? state.entities.selectedDetails
        }
      }));
      await refreshRuntime();
      await refreshTrace();
    }
  }

  function setEntitySearch(query: string): void {
    store.update((state) => ({ ...state, overview: { ...state.overview, query }, entities: { ...state.entities, query } }));
    if (searchTimer) {
      clearTimeout(searchTimer);
    }
    searchTimer = setTimeout(() => {
      void reopenEntityStream();
    }, searchDebounceMs);
  }

  function setEntityDomain(domain: DomainFilter): void {
    store.update((state) => ({ ...state, entities: { ...state.entities, domain } }));
    void reopenEntityStream();
  }

  function setEntitySource(source: SourceFilter): void {
    store.update((state) => ({ ...state, entities: { ...state.entities, source } }));
    void reopenEntityStream();
  }

  function selectOverviewItem(selectedItem: OverviewSelection): void {
    store.update((state) => ({
      ...state,
      overview: { ...state.overview, selectedItem }
    }));
  }

  async function launchClient(): Promise<void> {
    const project = get(store).project;
    if (!project) {
      return;
    }

    store.update((state) => ({ ...state, launchingClient: true }));
    const buildProfile = get(store).buildProfile;
    const result = await launchClientViewport(project.id, buildProfile);
    rememberCommandResult(result);
    store.update((state) => {
      const liveClientStatus = result.value ?? state.liveClientStatus;
      return {
      ...state,
      launchingClient: false,
      viewport: result.value ?? state.viewport,
      liveClientStatus,
      ...activeContextFields('live_client'),
      ...contextFields(
        state.previewRenderStatus.status,
        liveClientStatus?.status ?? 'stopped',
        state.serverRuntime?.process_status ?? 'stopped'
      ),
      diagnostics: appendDiagnostics(state.diagnostics, result.diagnostics)
      };
    });
    await refreshTrace();
  }

  async function stopClient(): Promise<void> {
    const viewport = get(store).liveClientStatus;
    if (!viewport) {
      return;
    }

    store.update((state) => ({ ...state, stoppingClient: true }));
    const sceneId = get(store).activeSceneId;
    const result = await stopClientViewport(viewport.id, sceneId);
    rememberCommandResult(result);
    store.update((state) => {
      const liveClientStatus = result.value?.kind === 'client' ? result.value : null;
      const previewStatus = state.previewRenderStatus;
      return {
      ...state,
      stoppingClient: false,
      viewport: result.value ?? state.viewport,
      liveClientStatus,
      previewRenderStatus: previewStatus,
      ...previewLifecycleFields(previewStatus),
      ...contextFields(
        previewStatus.status,
        liveClientStatus?.status ?? 'stopped',
        state.serverRuntime?.process_status ?? 'stopped'
      ),
      diagnostics: appendDiagnostics(state.diagnostics, result.diagnostics)
      };
    });
    await refreshTrace();
  }

  async function focusClient(): Promise<void> {
    const viewport = get(store).liveClientStatus;
    if (!viewport) {
      return;
    }

    const result = await focusClientViewport(viewport.id);
    rememberCommandResult(result);
    store.update((state) => ({
      ...state,
      viewport: result.value ?? state.viewport,
      diagnostics: appendDiagnostics(state.diagnostics, result.diagnostics)
    }));
  }

  async function reportViewportBounds(rect: ViewportRect): Promise<void> {
    const state = get(store);
    store.update((current) => ({ ...current, viewportRect: rect }));
    if (state.activeEditorContextId === 'preview') {
      if (state.project && state.previewRenderStatus.status !== 'idle') {
        await resizePreview(rect);
      }
      return;
    }
    if (state.activeEditorContextId !== 'live_client' || !state.liveClientStatus) {
      return;
    }

    const result = await resizeClientViewport(state.liveClientStatus.id, rect);
    rememberCommandResult(result);
    store.update((state) => ({
      ...state,
      viewport: result.value ?? state.viewport,
      liveClientStatus: result.value ?? state.liveClientStatus,
      diagnostics: appendDiagnostics(state.diagnostics, result.diagnostics)
    }));
  }

  async function ensureProjectPreview(
    reason: 'project_opened' | 'project_selected' | 'scene_changed' | 'editor_activated'
  ): Promise<void> {
    const state = get(store);
    if (!state.project) {
      return;
    }

    const requestToken = ++previewRequestToken;
    const rect = state.viewportRect ?? defaultViewportRect();
    const sceneId = state.activeSceneId ?? state.project.bsn_index.records[0]?.id ?? null;
    store.update((current) => {
      const preparing = {
        status: 'preparing' as PreviewLifecycleStatus,
        sceneId,
        frameIndex: current.previewRenderStatus.frameIndex,
        warning: null,
        error: null,
        renderer: current.previewRenderStatus.renderer
      };
      return {
        ...current,
        ...activeContextFields('preview'),
        previewRenderStatus: preparing,
        ...previewLifecycleFields(preparing),
        ...contextFields(
          'preparing',
          current.liveClientStatus?.status ?? 'stopped',
          current.serverRuntime?.process_status ?? 'stopped'
        )
      };
    });

    const result = await ensureEditorPreview(state.project.id, sceneId, rect);
    rememberCommandResult(result);
    if (requestToken !== previewRequestToken) {
      return;
    }
    const value = result.value;
    store.update((current) => {
      const previewStatus = previewStatusFromRenderer(
        value,
        sceneId,
        current.previewRenderStatus.frameIndex,
        result.diagnostics
      );
      return {
      ...current,
      viewportRect: rect,
      previewRenderStatus: previewStatus,
      ...previewLifecycleFields(previewStatus),
      ...contextFields(
        previewStatus.status,
        current.liveClientStatus?.status ?? 'stopped',
        current.serverRuntime?.process_status ?? 'stopped'
      ),
      diagnostics: appendDiagnostics(current.diagnostics, result.diagnostics)
      };
    });
  }

  async function resizePreview(rect: ViewportRect): Promise<void> {
    const state = get(store);
    if (!state.project) {
      return;
    }
    const result = await resizeEditorPreview(state.project.id, state.activeSceneId, rect);
    rememberCommandResult(result);
    store.update((current) => {
      const previewStatus = previewStatusFromRenderer(
        result.value,
        current.activeSceneId,
        current.previewRenderStatus.frameIndex,
        result.diagnostics
      );
      return {
        ...current,
        viewportRect: rect,
        previewRenderStatus: previewStatus,
        ...previewLifecycleFields(previewStatus),
        ...contextFields(
          previewStatus.status,
          current.liveClientStatus?.status ?? 'stopped',
          current.serverRuntime?.process_status ?? 'stopped'
        ),
        diagnostics: appendDiagnostics(current.diagnostics, result.diagnostics)
      };
    });
  }

  async function launchServer(): Promise<void> {
    const project = get(store).project;
    if (!project) {
      return;
    }

    store.update((state) => ({ ...state, launchingServer: true }));
    const result = await launchServerRuntime(project.id);
    rememberCommandResult(result);
    store.update((state) => {
      const serverRuntime = result.value ?? state.serverRuntime;
      return {
      ...state,
      launchingServer: false,
      serverRuntime,
      ...activeContextFields('server'),
      ...contextFields(
        state.previewRenderStatus.status,
        state.liveClientStatus?.status ?? 'stopped',
        serverRuntime?.process_status ?? 'stopped'
      ),
      diagnostics: appendDiagnostics(state.diagnostics, result.diagnostics)
      };
    });
    await refreshTrace();
  }

  async function refreshLauncher(): Promise<void> {
    const [launcherState, runtimeHostStatus, games, projects] = await Promise.all([
      getLauncherState(),
      getRuntimeHostStatus(),
      listGames(),
      listAuthorizedProjects()
    ]);
    store.update((state) => ({
      ...state,
      ...launcherStateToStore({
        ...launcherState,
        games,
        projects
      }),
      runtimeHostStatus
    }));
  }

  function setLauncherMode(launcherMode: LauncherMode): void {
    store.update((state) => ({ ...state, launcherMode }));
  }

  function selectGame(selectedGameId: string): void {
    store.update((state) => ({ ...state, selectedGameId }));
  }

  async function selectGameAndShowLauncher(selectedGameId: string): Promise<void> {
    selectGame(selectedGameId);
    await showLauncherShell();
  }

  function selectProject(selectedProjectId: string): void {
    store.update((state) => ({ ...state, selectedProjectId }));
  }

  async function showLauncherShell(): Promise<void> {
    const result = await showLauncher();
    rememberCommandResult(result);
    store.update((state) => ({
      ...state,
      ...(result.value ? launcherStateToStore(result.value) : { launcherMode: 'hidden' as LauncherMode }),
      hostMode: 'launcher',
      hostInputOwner: 'launcher_ui',
      diagnostics: appendDiagnostics(state.diagnostics, result.diagnostics)
    }));
  }

  async function hideLauncherShell(): Promise<void> {
    const result = await hideLauncher();
    rememberCommandResult(result);
    store.update((state) => ({
      ...state,
      ...(result.value ? launcherStateToStore(result.value) : { launcherMode: 'hidden' as LauncherMode }),
      hostMode: 'game',
      hostInputOwner: 'gameplay',
      diagnostics: appendDiagnostics(state.diagnostics, result.diagnostics)
    }));
  }

  async function joinSelectedGame(): Promise<void> {
    const gameId = get(store).selectedGameId;
    if (!gameId) {
      return;
    }
    const result = await joinGame(gameId);
    rememberCommandResult(result);
    store.update((state) => ({
      ...state,
      ...(result.value ? launcherStateToStore(result.value) : {}),
      diagnostics: appendDiagnostics(state.diagnostics, result.diagnostics)
    }));
    await refreshTrace();
  }

  async function hostLocal(): Promise<void> {
    await launchServer();
    await joinSelectedGame();
  }

  async function openSelectedProject(): Promise<void> {
    const selectedProject = get(store).projects.find((project) => project.id === get(store).selectedProjectId);
    if (!selectedProject) {
      return;
    }
    await openProjectAt(selectedProject.root_path);
    await refreshLauncher();
  }

  async function editSelectedProject(): Promise<void> {
    const projectId = get(store).selectedProjectId;
    if (!projectId) {
      return;
    }
    const result = await openProjectEditor(projectId);
    rememberCommandResult(result);
    store.update((state) => ({
      ...state,
      ...(result.value ? launcherStateToStore(result.value) : {}),
      diagnostics: appendDiagnostics(state.diagnostics, result.diagnostics)
    }));
    if (result.ok) {
      const current = get(store);
      if (current.project) {
        await ensureProjectPreview('project_selected');
      } else {
        const selectedProject = current.projects.find((project) => project.id === current.selectedProjectId);
        if (selectedProject) {
          await openProjectAt(selectedProject.root_path);
        }
      }
    }
    await refreshTrace();
  }

  async function activateEditorShell(): Promise<void> {
    const result = await activateEditor();
    rememberCommandResult(result);
    store.update((state) => ({
      ...state,
      ...(result.value ? launcherStateToStore(result.value) : { launcherMode: 'editor' as LauncherMode }),
      hostMode: 'editor',
      hostInputOwner: 'editor_ui',
      diagnostics: appendDiagnostics(state.diagnostics, result.diagnostics)
    }));
    const current = get(store);
    if (current.project && current.activeEditorContextId === 'preview') {
      await ensureProjectPreview('editor_activated');
    }
    await refreshTrace();
  }

  async function deactivateEditorShell(): Promise<void> {
    const result = await deactivateEditor();
    rememberCommandResult(result);
    store.update((state) => ({
      ...state,
      ...(result.value ? launcherStateToStore(result.value) : { launcherMode: 'launcher' as LauncherMode }),
      hostMode: 'game',
      hostInputOwner: 'gameplay',
      diagnostics: appendDiagnostics(state.diagnostics, result.diagnostics)
    }));
    await refreshTrace();
  }

  async function toggleEditorOverlayShell(): Promise<void> {
    const result = await toggleEditorOverlay();
    rememberCommandResult(result);
    store.update((state) => ({
      ...state,
      ...(result.value ? launcherStateToStore(result.value) : {}),
      hostMode: state.hostMode === 'editor_overlay' ? 'game' : 'editor_overlay',
      hostInputOwner: state.hostMode === 'editor_overlay' ? 'gameplay' : 'editor_ui',
      diagnostics: appendDiagnostics(state.diagnostics, result.diagnostics)
    }));
    await refreshTrace();
  }

  async function openGameMenu(): Promise<void> {
    const result = await setHostInputOwner('game_menu_ui');
    rememberCommandResult(result);
    store.update((state) => ({
      ...state,
      hostInputOwner: 'game_menu_ui',
      diagnostics: appendDiagnostics(state.diagnostics, result.diagnostics)
    }));
  }

  async function openGameChat(): Promise<void> {
    const result = await setHostInputOwner('text_entry');
    rememberCommandResult(result);
    store.update((state) => ({
      ...state,
      hostInputOwner: 'text_entry',
      diagnostics: appendDiagnostics(state.diagnostics, result.diagnostics)
    }));
  }

  async function stopActiveRuntime(): Promise<void> {
    if (get(store).viewport) {
      await stopClient();
      await refreshLauncher();
    }
  }

  function setProjectPath(projectPath: string): void {
    store.update((state) => ({ ...state, projectPath }));
  }

  function setCommandSearch(commandSearch: string): void {
    store.update((state) => ({ ...state, commandSearch }));
  }

  function toggleAccountPanel(): void {
    store.update((state) => ({
      ...state,
      account: {
        ...state.account,
        loginOpen: !state.account.loginOpen,
        error: null
      }
    }));
  }

  function closeAccountPanel(): void {
    store.update((state) => ({
      ...state,
      account: {
        ...state.account,
        loginOpen: false,
        error: null
      }
    }));
  }

  const backendCapabilities = ['read_entities', 'read_diagnostics', 'control_runtime'] as const;

  async function requestAccountTicket(
    email: string,
    password: string,
    mode: 'login' | 'register' = 'login',
    displayName?: string
  ): Promise<void> {
    store.update((state) => ({
      ...state,
      account: {
        ...state.account,
        loading: true,
        error: null
      }
    }));
    const result = await requestBackendAccountTicket({
      email,
      password,
      mode,
      display_name: mode === 'register' ? displayName ?? null : null,
      audience: 'fun_editor',
      requested_capabilities: [...backendCapabilities]
    });
    rememberCommandResult(result);
    store.update((state) => {
      const diagnostics = appendDiagnostics(state.diagnostics, result.diagnostics);
      if (!result.ok || !result.value) {
        return {
          ...state,
          diagnostics,
          account: {
            ...state.account,
            loading: false,
            error: result.diagnostics[0]?.message ?? 'Auth ticket request failed.'
          }
        };
      }
      return {
        ...state,
        diagnostics,
        account: {
          ...state.account,
          loading: false,
          loginOpen: false,
          backendUrl: result.value.backend_base_url,
          profile: result.value.profile,
          ticket: result.value,
          error: null
        }
      };
    });
  }

  async function refreshAccountTicket(): Promise<void> {
    store.update((state) => ({
      ...state,
      account: {
        ...state.account,
        loading: true,
        error: null
      }
    }));
    const result = await requestBackendAuthTicket({
      audience: 'fun_editor',
      requested_capabilities: [...backendCapabilities]
    });
    rememberCommandResult(result);
    store.update((state) => {
      const diagnostics = appendDiagnostics(state.diagnostics, result.diagnostics);
      if (!result.ok || !result.value) {
        return {
          ...state,
          diagnostics,
          account: {
            ...state.account,
            loading: false,
            error: result.diagnostics[0]?.message ?? 'Auth ticket refresh failed.'
          }
        };
      }
      return {
        ...state,
        diagnostics,
        account: {
          ...state.account,
          loading: false,
          backendUrl: result.value.backend_base_url,
          profile: result.value.profile,
          ticket: result.value,
          error: null
        }
      };
    });
  }

  async function logoutAccount(): Promise<void> {
    store.update((state) => ({
      ...state,
      account: {
        ...state.account,
        loading: true,
        error: null
      }
    }));
    const result = await logoutBackendAccount();
    rememberCommandResult(result);
    store.update((state) => ({
      ...state,
      diagnostics: appendDiagnostics(state.diagnostics, result.diagnostics),
      account: {
        ...state.account,
        loading: false,
        profile: null,
        ticket: null,
        error: result.ok ? null : result.diagnostics[0]?.message ?? 'Logout failed.'
      }
    }));
  }

  function setCommandbarInput(input: string): void {
    if (isHostRuntime()) {
      void setHostInputOwner('commandbar');
    }
    store.update((state) => ({
      ...state,
      commandbar: commandbarForState(state, input, { focused: true })
    }));
  }

  function focusCommandbar(): void {
    if (isHostRuntime()) {
      void setHostInputOwner('commandbar');
    }
    store.update((state) => ({
      ...state,
      commandbar: commandbarForState(state, state.commandbar.input, { focused: true })
    }));
  }

  function blurCommandbar(): void {
    if (isHostRuntime()) {
      const state = get(store);
      const owner = state.launcherMode === 'editor' ? 'editor_ui' : state.launcherMode === 'launcher' ? 'launcher_ui' : 'gameplay';
      void setHostInputOwner(owner);
    }
    store.update((state) => ({
      ...state,
      commandbar: {
        ...commandbarForState(state, state.commandbar.input),
        focused: false,
        mode: state.commandbar.input.trim() ? state.commandbar.mode : 'identity'
      }
    }));
  }

  function clearCommandbar(): void {
    store.update((state) => ({
      ...state,
      commandbar: {
        ...commandbarForState(state, ''),
        focused: state.commandbar.focused,
        input: '',
        mode: 'identity'
      }
    }));
  }

  function moveCommandbarSelection(delta: number): void {
    store.update((state) => {
      const results = state.commandbar.results;
      if (results.length === 0) {
        return state;
      }
      const currentIndex = Math.max(
        0,
        results.findIndex((result) => result.id === state.commandbar.selectedResultId)
      );
      const nextIndex = (currentIndex + delta + results.length) % results.length;
      return {
        ...state,
        commandbar: {
          ...state.commandbar,
          selectedResultId: results[nextIndex]?.id
        }
      };
    });
  }

  function setCommandbarScope(activeScope: ExperienceScope): void {
    store.update((state) => {
      const nextState = {
        ...state,
        commandbar: { ...state.commandbar, activeScope }
      };
      return {
        ...nextState,
        commandbar: commandbarForState(nextState, state.commandbar.input, { activeScope })
      };
    });
  }

  async function executeCommandbarResult(resultId?: string): Promise<void> {
    const state = get(store);
    const result =
      state.commandbar.results.find((candidate) => candidate.id === (resultId ?? state.commandbar.selectedResultId)) ??
      state.commandbar.results[0];
    if (!result?.action) {
      return;
    }

    await executeCommandbarAction(result.action);
  }

  async function executeCommandbarAction(action: CommandbarAction): Promise<void> {
    const hostExecution = await executeHostCommandbarAction(action);
    rememberCommandResult(hostExecution);
    if (hostExecution.diagnostics.length > 0 || !hostExecution.ok) {
      store.update((state) => ({
        ...state,
        diagnostics: appendDiagnostics(state.diagnostics, hostExecution.diagnostics),
        error: hostExecution.ok ? state.error : hostExecution.diagnostics[0]?.message ?? 'Commandbar command rejected by host.'
      }));
    }
    if (!hostExecution.ok) {
      return;
    }

    if (action.kind === 'tool_call_preview') {
      const preview = toolCallPreview(action.toolCall);
      store.update((state) => ({
        ...state,
        commandbar: {
          ...state.commandbar,
          pendingToolCall: preview
        }
      }));
      if (!action.toolCall.requiresConfirmation) {
        rememberCommandResult({
          ok: true,
          value: { toolName: action.toolCall.toolName, arguments: action.toolCall.arguments },
          diagnostics: []
        });
      }
      return;
    }

    if (action.kind === 'filter_diagnostics') {
      if (action.severity) {
        toggleDiagnosticsFilter('severity', action.severity);
      }
      if (action.tag) {
        toggleDiagnosticsFilter('tag', action.tag);
      }
      setDiagnosticsVisible(true);
      return;
    }

    if (action.kind === 'set_context') {
      setActiveEditorContext(action.contextId);
      return;
    }

    if (action.kind === 'open_project') {
      await openProjectAt(action.path);
      return;
    }

    if (action.kind === 'select_entity') {
      const row = Object.values(get(store).entities.rowsByIndex).find((entity) => entity.id === action.entityId);
      if (row) {
        await selectEntity(row);
      }
      return;
    }

    if (action.kind !== 'command') {
      return;
    }

    switch (action.commandId) {
      case 'editor.open_default_project':
        await openDefaultProject();
        break;
      case 'editor.pick_project':
        await pickProject();
        break;
      case 'editor.reload_preview':
        await reloadPreview();
        break;
      case 'editor.launch_client':
        await launchClient();
        break;
      case 'editor.launch_server':
        await launchServer();
        break;
      case 'editor.stop_runtime':
        await stopActiveRuntime();
        break;
      case 'editor.show_diagnostics':
        setDiagnosticsVisible(true);
        break;
      case 'editor.hide_diagnostics':
        setDiagnosticsVisible(false);
        break;
      case 'editor.set_context.preview':
        setActiveEditorContext('preview');
        break;
      case 'editor.set_context.graph':
        setActiveEditorContext('graph');
        break;
      case 'editor.set_context.diagnostics':
        setActiveEditorContext('diagnostics');
        break;
      case 'editor.show_log':
        setActiveEditorContext('diagnostics');
        break;
      case 'editor.set_context.overview':
        setActiveEditorContext('overview');
        break;
      case 'editor.frame_viewport':
        await fitViewport();
        break;
      default:
        rememberCommandResult({
          ok: true,
          value: { commandId: action.commandId, routed: 'commandbar' },
          diagnostics: []
        });
        break;
    }
  }

  function setDiagnosticsVisible(visible: boolean): void {
    store.update((state) => {
      persistDiagnosticsVisible(state, visible);
      return {
        ...state,
        diagnosticsView: {
          ...state.diagnosticsView,
          visible
        }
      };
    });
  }

  function setDiagnosticsHeight(height: number): void {
    store.update((state) => ({
      ...state,
      diagnosticsView: {
        ...state.diagnosticsView,
        height: Math.round(Math.max(96, height))
      }
    }));
  }

  function toggleDiagnostics(): void {
    setDiagnosticsVisible(!get(store).diagnosticsView.visible);
  }

  function toggleDiagnosticsFilter(kind: 'severity' | 'tag' | 'source' | 'scope', value: string): void {
    store.update((state) => {
      const activeFilters = state.diagnosticsView.activeFilters;
      const key =
        kind === 'severity' ? 'severities' : kind === 'tag' ? 'tags' : kind === 'source' ? 'sources' : 'scopes';
      const values = activeFilters[key] as string[];
      const nextValues = values.includes(value)
        ? values.filter((candidate) => candidate !== value)
        : [...values, value];
      return {
        ...state,
        diagnosticsView: {
          ...state.diagnosticsView,
          activeFilters: {
            ...activeFilters,
            [key]: nextValues
          }
        }
      };
    });
  }

  function clearDiagnosticsFilters(): void {
    store.update((state) => ({
      ...state,
      diagnosticsView: {
        ...state.diagnosticsView,
        activeFilters: emptyDiagnosticsFilters
      }
    }));
  }

  function setDiagnosticsSortMode(sortMode: DiagnosticsSortMode): void {
    store.update((state) => ({
      ...state,
      diagnosticsView: {
        ...state.diagnosticsView,
        sortMode
      }
    }));
  }

  function setDiagnosticsFilterMode(filterMode: EditorUiState['diagnosticsView']['filterMode']): void {
    store.update((state) => ({
      ...state,
      diagnosticsView: {
        ...state.diagnosticsView,
        filterMode
      }
    }));
  }

  function setActiveScene(activeSceneId: string): void {
    store.update((state) => ({ ...state, activeSceneId }));
    if (get(store).activeEditorContextId === 'preview') {
      void ensureProjectPreview('scene_changed');
    } else {
      const state = get(store);
      if (state.project) {
        void updatePreviewSceneInBackground(state.project.id, activeSceneId);
      }
    }
  }

  async function updatePreviewSceneInBackground(projectId: string, sceneId: string): Promise<void> {
    const result = await setEditorPreviewScene(projectId, sceneId);
    rememberCommandResult(result);
    store.update((current) => {
      const previewStatus = previewStatusFromRenderer(
        result.value,
        sceneId,
        current.previewRenderStatus.frameIndex,
        result.diagnostics
      );
      return {
        ...current,
        previewRenderStatus: previewStatus,
        ...previewLifecycleFields(previewStatus),
        ...contextFields(
          previewStatus.status,
          current.liveClientStatus?.status ?? 'stopped',
          current.serverRuntime?.process_status ?? 'stopped'
        ),
        diagnostics: appendDiagnostics(current.diagnostics, result.diagnostics)
      };
    });
  }

  function setActiveEditorContext(activeEditorContextId: EditorContextId): void {
    store.update((state) => ({ ...state, ...activeContextFields(activeEditorContextId), contextSwitching: true }));
    if (activeEditorContextId === 'preview') {
      void ensureProjectPreview('editor_activated');
    } else if (activeEditorContextId === 'diagnostics') {
      store.update((state) => {
        persistDiagnosticsVisible(state, true);
        return {
          ...state,
          bottomTab: 'diagnostics',
          diagnosticsView: { ...state.diagnosticsView, visible: false },
          contextSwitching: false
        };
      });
    } else {
      store.update((state) => ({ ...state, contextSwitching: false }));
    }
  }

  function setBuildProfile(buildProfile: BuildProfile): void {
    store.update((state) => ({ ...state, buildProfile }));
    if (buildProfile === 'debug_diagnostics') {
      store.update((state) => ({ ...state, bottomTab: 'frame_profile' }));
    }
  }

  function setRenderMode(renderMode: RenderMode): void {
    store.update((state) => ({ ...state, renderMode }));
  }

  function setBottomTab(bottomTab: EditorUiState['bottomTab']): void {
    store.update((state) => ({ ...state, bottomTab }));
  }

  async function restartClient(): Promise<void> {
    await stopClient();
    await launchClient();
  }

  async function fitViewport(): Promise<void> {
    if (get(store).activeEditorContextId === 'preview') {
      await refreshPreviewFrame();
      return;
    }
    await refreshRuntime();
  }

  async function refreshPreviewFrame(): Promise<void> {
    const result = await getEditorPreviewFrame();
    rememberCommandResult(result);
    store.update((current) => {
      const previewStatus = previewStatusFromRenderer(
        result.value,
        current.activeSceneId,
        current.previewRenderStatus.frameIndex,
        result.diagnostics
      );
      return {
        ...current,
        previewRenderStatus: previewStatus,
        ...previewLifecycleFields(previewStatus),
        diagnostics: appendDiagnostics(current.diagnostics, result.diagnostics)
      };
    });
  }

  async function reloadPreview(): Promise<void> {
    await ensureProjectPreview('scene_changed');
  }

  async function resetPreviewCamera(): Promise<void> {
    await refreshPreviewFrame();
  }

  function openCommandPalette(): void {
    store.update((state) => ({
      ...state,
      commandbar: commandbarForState(state, state.commandbar.input, { focused: true, mode: 'command' })
    }));
  }

  return {
    subscribe: store.subscribe,
    initialize,
    refreshTrace,
    refreshRuntime,
    openDefaultProject,
    openTypedProject,
    openProjectAt,
    pickProject,
    ensureEntityRange,
    selectEntity,
    patchSelectedTransform,
    setEntitySearch,
    setEntityDomain,
    setEntitySource,
    selectOverviewItem,
    launchClient,
    stopClient,
    restartClient,
    focusClient,
    fitViewport,
    ensureProjectPreview,
    reloadPreview,
    resetPreviewCamera,
    reportViewportBounds,
    launchServer,
    refreshLauncher,
    setLauncherMode,
    selectGame,
    selectGameAndShowLauncher,
    selectProject,
    showLauncherShell,
    hideLauncherShell,
    joinSelectedGame,
    hostLocal,
    openSelectedProject,
    editSelectedProject,
    activateEditorShell,
    deactivateEditorShell,
    toggleEditorOverlayShell,
    openGameMenu,
    openGameChat,
    stopActiveRuntime,
    setProjectPath,
    setCommandSearch,
    toggleAccountPanel,
    closeAccountPanel,
    requestAccountTicket,
    refreshAccountTicket,
    logoutAccount,
    setCommandbarInput,
    focusCommandbar,
    blurCommandbar,
    clearCommandbar,
    moveCommandbarSelection,
    setCommandbarScope,
    executeCommandbarResult,
    setDiagnosticsVisible,
    setDiagnosticsHeight,
    toggleDiagnostics,
    toggleDiagnosticsFilter,
    clearDiagnosticsFilters,
    setDiagnosticsSortMode,
    setDiagnosticsFilterMode,
    setActiveScene,
    setActiveEditorContext,
    setBuildProfile,
    setRenderMode,
    setBottomTab,
    openCommandPalette
  };
}

function entityFilter(domain: DomainFilter, source: SourceFilter, query: string): EntityFilter {
  return {
    domain: domain === 'all' ? null : domain,
    source: source === 'both' ? 'both' : source,
    query: query.trim() || null
  };
}

function errorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
}

function defaultViewportRect(): ViewportRect {
  return {
    x: 0,
    y: 0,
    width: 1280,
    height: 720,
    scale_factor: typeof window === 'undefined' ? 1 : window.devicePixelRatio || 1
  };
}

export const editorStore = createEditorStore();

function editorContexts(
  previewStatus: string,
  liveClientStatus: string,
  serverStatus: string
): EditorUiState['editorContexts'] {
  return [
    { id: 'overview', label: 'Overview', status: 'ready' },
    { id: 'preview', label: 'Preview', status: previewStatus },
    { id: 'live_client', label: 'Current Client', status: liveClientStatus },
    { id: 'server', label: 'Server', status: serverStatus },
    { id: 'graph', label: 'Graph', status: 'ready' },
    { id: 'diagnostics', label: 'Log', status: 'ready' }
  ];
}

function contextFields(
  previewStatus: string,
  liveClientStatus: string,
  serverStatus: string
): Pick<EditorUiState, 'editorContexts' | 'availableContexts'> {
  const contexts = editorContexts(previewStatus, liveClientStatus, serverStatus);
  return {
    editorContexts: contexts,
    availableContexts: contexts
  };
}

function activeContextFields(
  editorContext: EditorContextId
): Pick<EditorUiState, 'activeEditorContextId' | 'editorContext' | 'contextSwitching'> {
  return {
    activeEditorContextId: editorContext,
    editorContext,
    contextSwitching: false
  };
}

function overviewSelectionFromEntity(row: EntityRowSummary): OverviewSelection {
  return {
    kind: 'entity',
    id: row.id,
    title: row.name,
    subtitle: `${row.domain} ${row.source_kind}`,
    path: row.source_span?.path ?? row.source,
    scope: row.domain === 'client' ? { type: 'all_connected' } : { type: 'all_projects' },
    metadata: {
      domain: row.domain,
      source: row.source,
      sourceKind: row.source_kind,
      components: row.component_count,
      live: row.live_snapshot,
      revision: row.revision,
      selectable: row.selectable
    }
  };
}

function appendDiagnostics(existing: Diagnostic[], next: Diagnostic[]): Diagnostic[] {
  if (next.length === 0) {
    return existing;
  }
  const merged = [...existing, ...next];
  const seen = new Set<string>();
  const deduped: Diagnostic[] = [];
  for (let index = merged.length - 1; index >= 0; index -= 1) {
    const diagnostic = merged[index];
    const key = `${diagnostic.code}\n${diagnostic.level}\n${diagnostic.message}\n${diagnostic.hosted_instance_id ?? ''}`;
    if (seen.has(key)) {
      continue;
    }
    seen.add(key);
    deduped.push(diagnostic);
    if (deduped.length >= diagnosticsCap) {
      break;
    }
  }
  deduped.reverse();
  return deduped;
}

function diagnosticsViewWithCounts(
  diagnosticsView: EditorUiState['diagnosticsView'],
  diagnostics: Diagnostic[]
): EditorUiState['diagnosticsView'] {
  const counts = diagnostics.reduce(
    (accumulator, diagnostic) => {
      if (diagnostic.level === 'error') {
        accumulator.unreadCriticalCount += 1;
      } else if (diagnostic.level === 'warning') {
        accumulator.unreadWarningCount += 1;
      } else {
        accumulator.unreadInfoCount += 1;
      }
      return accumulator;
    },
    { unreadCriticalCount: 0, unreadWarningCount: 0, unreadInfoCount: 0 }
  );
  return {
    ...diagnosticsView,
    ...counts
  };
}

function diagnosticsViewForProject(
  diagnosticsView: EditorUiState['diagnosticsView'],
  projectKey: string | null,
  diagnostics: Diagnostic[]
): EditorUiState['diagnosticsView'] {
  return {
    ...diagnosticsViewWithCounts(diagnosticsView, diagnostics),
    visible: readDiagnosticsVisible(projectKey, diagnosticsView.visible)
  };
}

function diagnosticsPreferenceKey(projectKey: string | null): string {
  return `${diagnosticsPreferencePrefix}:${projectKey?.trim() || 'workspace'}`;
}

function readDiagnosticsVisible(projectKey: string | null, fallback: boolean): boolean {
  if (typeof window === 'undefined') {
    return fallback;
  }
  try {
    const stored = window.localStorage.getItem(diagnosticsPreferenceKey(projectKey));
    return stored === null ? fallback : stored === 'true';
  } catch {
    return fallback;
  }
}

function persistDiagnosticsVisible(state: EditorUiState, visible: boolean): void {
  if (typeof window === 'undefined') {
    return;
  }
  try {
    window.localStorage.setItem(diagnosticsPreferenceKey(state.project?.id ?? state.projectPath), String(visible));
  } catch {
    // Local storage can be unavailable in hardened webviews; the in-memory state still works.
  }
}

function commandbarForState(
  state: EditorUiState,
  input: string,
  overrides: Partial<EditorUiState['commandbar']> = {}
): EditorUiState['commandbar'] {
  const trimmed = input.trim();
  const activeScope =
    overrides.activeScope ??
    (state.commandbar.activeScope?.type === 'all_projects' && state.project
      ? defaultExperienceScope(state)
      : state.commandbar.activeScope ?? defaultExperienceScope(state));
  const mode = overrides.mode ?? inferCommandbarMode(trimmed);
  const routedIntent = routeCommandbarIntent(trimmed, state, activeScope);
  const results = trimmed ? buildCommandbarResults(state, trimmed, activeScope, commandbarResultCap, routedIntent) : [];
  const selectedResultId =
    overrides.selectedResultId ??
    (results.some((result) => result.id === state.commandbar.selectedResultId)
      ? state.commandbar.selectedResultId
      : results[0]?.id);
  return {
    ...state.commandbar,
    ...overrides,
    input,
    mode,
    activeScope,
    selectedResultId,
    results,
    pendingToolCall: trimmed === state.commandbar.input ? state.commandbar.pendingToolCall : undefined,
    inferredIntent: intentSummary(routedIntent),
    modelRoute: routedIntent.needsLlm ? 'local' : mode === 'tool' ? 'mcp' : 'none'
  };
}

function renderSignatureWarning(diagnostics: Diagnostic[]): string | null {
  return diagnostics.some((diagnostic) => diagnostic.code === 'preview.renderer.winit_feature_leak')
    ? 'render signature mismatch'
    : null;
}

function previewStatusFromRenderer(
  renderer: PreviewRendererStatus | null,
  requestedSceneId: string | null,
  fallbackFrameIndex: number,
  diagnostics: Diagnostic[]
): EditorUiState['previewRenderStatus'] {
  if (!renderer) {
    return {
      status: 'failed',
      sceneId: requestedSceneId,
      frameIndex: fallbackFrameIndex,
      warning: renderSignatureWarning(diagnostics),
      error: diagnostics.find((diagnostic) => diagnostic.level === 'error')?.message ?? 'Preview renderer did not return a frame.',
      renderer: null
    };
  }
  const failed = renderer.status === 'failed';
  return {
    status: failed ? 'failed' : 'ready',
    sceneId: renderer.scene.scene_id || requestedSceneId,
    frameIndex: renderer.frame.frame_index,
    warning: renderer.signature_warning ?? renderSignatureWarning(diagnostics),
    error: failed ? renderer.signature_warning ?? 'Preview renderer failed.' : null,
    renderer
  };
}

function previewLifecycleFields(
  previewRenderStatus: EditorUiState['previewRenderStatus']
): Pick<
  EditorUiState,
  'previewStatus' | 'previewError' | 'previewFrameRevision' | 'previewSceneId'
> {
  return {
    previewStatus: previewRenderStatus.status,
    previewError: previewRenderStatus.error,
    previewFrameRevision: previewRenderStatus.frameIndex,
    previewSceneId: previewRenderStatus.sceneId
  };
}

function launcherStateToStore(launcherState: LauncherState): Pick<
  EditorUiState,
  | 'launcherMode'
  | 'games'
  | 'projects'
  | 'selectedGameId'
  | 'selectedProjectId'
  | 'activeRuntime'
  | 'authorization'
  | 'runtimeHostStatus'
> {
  return {
    launcherMode: launcherState.launcher_mode,
    games: launcherState.games,
    projects: launcherState.projects,
    selectedGameId: launcherState.selected_game_id,
    selectedProjectId: launcherState.selected_project_id,
    activeRuntime: launcherState.active_runtime,
    authorization: launcherState.authorization,
    runtimeHostStatus: launcherState.runtime_host_status
  };
}
