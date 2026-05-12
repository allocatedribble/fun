import { emitHost, isHostRuntime } from '../host/commands';
import type { Diagnostic, EditorEvent, EditorUiState } from '../types';

export const RVELTE_DEV_ISLAND_ENABLED = import.meta.env.VITE_FUN_RVELTE_DEV === '1';
export const RVELTE_DEV_ASSET_BASE = '/rvelte-dev/';
export const RVELTE_DEV_PANEL_MODULE_URL = `${RVELTE_DEV_ASSET_BASE}dev-panel.js`;
export const RVELTE_DEV_PANEL_WASM_URL = `${RVELTE_DEV_ASSET_BASE}dev_panel.wasm`;
export const RVELTE_DEV_PANEL_CSS_URL = `${RVELTE_DEV_ASSET_BASE}generated/component.scope.css`;
export const RVELTE_DEV_BRIDGE_SCHEMA_VERSION = 'fun.rvelte.dev_bridge.v1';
export const RVELTE_HOST_BRIDGE_SCHEMA_VERSION = 'rvelte.host_bridge.v1';
export const RVELTE_DEV_PANEL_COMPONENT_ID = 'diagnostics_log_panel';
export const RVELTE_DEV_EXPECTED_MANIFEST_ID = 'rvelte.manifest.diagnostics_log_panel_component.v1';
export const RVELTE_DEV_EXPECTED_FACADE_ABI = 1;

const source = 'svelte_diagnostics_shell';
const maxRows = 8;
const maxHostPayloadBytes = 4096;
let nextEventSequence = 1;
let nextHostCorrelationSequence = 1;
let nextSnapshotRevision = 1;

export type RvelteDevIslandDiagnosticCode =
  | 'rvelte.wasm_missing'
  | 'rvelte.manifest_mismatch'
  | 'rvelte.facade_incompatible'
  | 'rvelte.host_bridge_unavailable'
  | 'rvelte.host_payload_invalid';

export type RvelteDevIslandDiagnostic = {
  code: RvelteDevIslandDiagnosticCode;
  severity: 'info' | 'warn' | 'error';
  message: string;
  detail: string;
  next_action: string;
};

export type RvelteDevPanelRow = {
  id: string;
  level: 'info' | 'warning' | 'error';
  group: 'diagnostic' | 'runtime' | 'event' | 'command';
  source: string;
  message: string;
  detail: string;
};

export type RvelteDevPanelSnapshot = {
  revision: number;
  errors: number;
  warnings: number;
  totalRows: number;
  runtimeLabel: string;
  frameLabel: string;
  rows: RvelteDevPanelRow[];
};

export type RvelteDevMountMessage = {
  schema_version: typeof RVELTE_DEV_BRIDGE_SCHEMA_VERSION;
  kind: 'rvelte_diagnostics_log_panel_mount';
  component_id: typeof RVELTE_DEV_PANEL_COMPONENT_ID;
  manifest_id: typeof RVELTE_DEV_EXPECTED_MANIFEST_ID;
  source: typeof source;
  runtime_route: 'editor.diagnostics';
  host_bridge_available: boolean;
  snapshot: RvelteDevPanelSnapshot;
};

export type RvelteDevPanelEvent = {
  schema_version: typeof RVELTE_DEV_BRIDGE_SCHEMA_VERSION;
  kind: 'rvelte_dev_panel_refresh_requested';
  component_id: typeof RVELTE_DEV_PANEL_COMPONENT_ID;
  manifest_id: typeof RVELTE_DEV_EXPECTED_MANIFEST_ID;
  source: 'rvelte_diagnostics_log_panel';
  sequence: number;
  revision: number;
};

export type RvelteHostCommandId = 'diagnostics.snapshot.get' | 'diagnostics.refresh';

export type RvelteHostBridgeRequest = {
  schema_version: typeof RVELTE_HOST_BRIDGE_SCHEMA_VERSION;
  kind: 'rvelte_host_command_request';
  component_id: typeof RVELTE_DEV_PANEL_COMPONENT_ID;
  manifest_id: typeof RVELTE_DEV_EXPECTED_MANIFEST_ID;
  source: 'rvelte_diagnostics_log_panel';
  command_id: RvelteHostCommandId;
  correlation_id: string;
  request_revision: number;
  freshness_revision: number;
  payload_size_bytes: number;
  authorization_context: {
    authority: 'host_validated_required';
    requested_capability: 'ReadDiagnostics' | 'RefreshDiagnostics';
  };
  payload:
    | {
        current_revision: number;
        max_rows: typeof maxRows;
      }
    | {
        current_revision: number;
        interaction_revision: number;
      };
};

export type RvelteDevPanelInstance = {
  snapshot: () => RvelteDevPanelSnapshot;
  update: (snapshot: RvelteDevPanelSnapshot) => unknown;
  destroy: () => void;
};

type RvelteDevPanelModule = {
  RVELTE_BROWSER_DEV_PANEL_FACADE_ABI: number;
  RVELTE_BROWSER_DEV_PANEL_MANIFEST_ID: string;
  mountBrowserDevPanelIsland: (options: {
    root: HTMLElement;
    status: HTMLElement;
    wasmUrl: string;
    updateDocumentTitle: false;
    snapshot: RvelteDevPanelSnapshot;
    onRefreshRequested: (event: unknown) => void;
  }) => Promise<RvelteDevPanelInstance>;
};

export type RvelteDevMountResult = {
  instance: RvelteDevPanelInstance | null;
  diagnostics: RvelteDevIslandDiagnostic[];
};

const diagnosticCatalog = {
  'rvelte.wasm_missing': {
    severity: 'error',
    message: 'rvelte Wasm fixture is unavailable.',
    detail: 'asset=/rvelte-dev/dev_panel.wasm',
    next_action: 'Regenerate the dev panel assets before enabling the flag.'
  },
  'rvelte.manifest_mismatch': {
    severity: 'error',
    message: 'rvelte manifest does not match the Svelte bridge contract.',
    detail: `expected_manifest=${RVELTE_DEV_EXPECTED_MANIFEST_ID}`,
    next_action: 'Regenerate DiagnosticsLogPanel.rvt outputs from the current manifest.'
  },
  'rvelte.facade_incompatible': {
    severity: 'error',
    message: 'rvelte facade ABI is incompatible with this UI bridge.',
    detail: `expected_abi=${RVELTE_DEV_EXPECTED_FACADE_ABI}`,
    next_action: 'Rebuild the rvelte dev panel assets with the matching facade.'
  },
  'rvelte.host_bridge_unavailable': {
    severity: 'warn',
    message: 'Fun host bridge is unavailable.',
    detail: 'host_bridge=missing',
    next_action: 'The panel can run locally, but host forwarding is disabled.'
  },
  'rvelte.host_payload_invalid': {
    severity: 'error',
    message: 'rvelte host bridge payload was rejected.',
    detail: `schema=${RVELTE_DEV_BRIDGE_SCHEMA_VERSION}`,
    next_action: 'Fix the typed bridge payload before mutating rvelte state.'
  }
} satisfies Record<RvelteDevIslandDiagnosticCode, Omit<RvelteDevIslandDiagnostic, 'code'>>;

export function rvelteDevDiagnostic(code: RvelteDevIslandDiagnosticCode): RvelteDevIslandDiagnostic {
  return { code, ...diagnosticCatalog[code] };
}

export function createRvelteDevPanelSnapshot(state: EditorUiState): RvelteDevPanelSnapshot {
  const rows = buildRows(state.diagnostics, state.events, state).slice(0, maxRows);
  const errors = rows.filter((row) => row.level === 'error').length;
  const warnings = rows.filter((row) => row.level === 'warning').length;
  const runtime = state.runtimeStatus ?? state.status?.runtime_status ?? null;
  const frame = state.runtimeDiagnostics?.latest_frame_profile ?? null;
  return {
    revision: nextSnapshotRevision++,
    errors: clampDiagnosticCount(errors),
    warnings: clampDiagnosticCount(warnings),
    totalRows: clampDiagnosticCount(
      state.diagnostics.length + state.events.length + (state.runtimeDiagnostics?.records.length ?? 0)
    ),
    runtimeLabel: compactText(
      `client ${runtime?.client_process.state ?? 'unknown'} / server ${runtime?.server_process.state ?? 'unknown'}`,
      64
    ),
    frameLabel: frame
      ? compactText(`frame ${frame.frame} / gpu ${Math.round(frame.gpu_ns / 1000)} us`, 64)
      : 'frame n/a',
    rows
  };
}

export function createRvelteDevMountMessage(state: EditorUiState): RvelteDevMountMessage {
  return {
    schema_version: RVELTE_DEV_BRIDGE_SCHEMA_VERSION,
    kind: 'rvelte_diagnostics_log_panel_mount',
    component_id: RVELTE_DEV_PANEL_COMPONENT_ID,
    manifest_id: RVELTE_DEV_EXPECTED_MANIFEST_ID,
    source,
    runtime_route: 'editor.diagnostics',
    host_bridge_available: isHostRuntime(),
    snapshot: createRvelteDevPanelSnapshot(state)
  };
}

export async function mountRvelteDevPanel(
  root: HTMLElement,
  status: HTMLElement,
  message: unknown,
  onRefreshRequested: (revision: number) => void
): Promise<RvelteDevMountResult> {
  const diagnostics: RvelteDevIslandDiagnostic[] = [];
  const payload = validateMountMessage(message);
  if (payload === null) {
    return { instance: null, diagnostics: [rvelteDevDiagnostic('rvelte.host_payload_invalid')] };
  }
  if (!payload.host_bridge_available) {
    diagnostics.push(rvelteDevDiagnostic('rvelte.host_bridge_unavailable'));
  }

  const module = await importDevPanelModule();
  if (module === null) {
    return { instance: null, diagnostics: [...diagnostics, rvelteDevDiagnostic('rvelte.facade_incompatible')] };
  }
  if (module.RVELTE_BROWSER_DEV_PANEL_MANIFEST_ID !== RVELTE_DEV_EXPECTED_MANIFEST_ID) {
    return { instance: null, diagnostics: [...diagnostics, rvelteDevDiagnostic('rvelte.manifest_mismatch')] };
  }
  if (module.RVELTE_BROWSER_DEV_PANEL_FACADE_ABI !== RVELTE_DEV_EXPECTED_FACADE_ABI) {
    return { instance: null, diagnostics: [...diagnostics, rvelteDevDiagnostic('rvelte.facade_incompatible')] };
  }

  try {
    const instance = await module.mountBrowserDevPanelIsland({
      root,
      status,
      wasmUrl: RVELTE_DEV_PANEL_WASM_URL,
      updateDocumentTitle: false,
      snapshot: payload.snapshot,
      onRefreshRequested: (event) => forwardRefreshRequested(payload, event, onRefreshRequested)
    });
    return { instance, diagnostics };
  } catch (error) {
    return { instance: null, diagnostics: [...diagnostics, diagnosticFromMountError(error)] };
  }
}

export function updateRvelteDevPanelSnapshot(
  instance: RvelteDevPanelInstance | null,
  snapshot: RvelteDevPanelSnapshot
): boolean {
  if (instance === null || !validateSnapshot(snapshot)) {
    return false;
  }
  instance.update(snapshot);
  return true;
}

async function importDevPanelModule(): Promise<RvelteDevPanelModule | null> {
  try {
    const module = (await import(/* @vite-ignore */ RVELTE_DEV_PANEL_MODULE_URL)) as unknown;
    return isDevPanelModule(module) ? module : null;
  } catch {
    return null;
  }
}

function isDevPanelModule(value: unknown): value is RvelteDevPanelModule {
  if (!isRecord(value)) {
    return false;
  }
  return (
    typeof value.RVELTE_BROWSER_DEV_PANEL_FACADE_ABI === 'number' &&
    typeof value.RVELTE_BROWSER_DEV_PANEL_MANIFEST_ID === 'string' &&
    typeof value.mountBrowserDevPanelIsland === 'function'
  );
}

function validateMountMessage(value: unknown): RvelteDevMountMessage | null {
  if (!isRecord(value)) {
    return null;
  }
  if (
    value.schema_version !== RVELTE_DEV_BRIDGE_SCHEMA_VERSION ||
    value.kind !== 'rvelte_diagnostics_log_panel_mount' ||
    value.component_id !== RVELTE_DEV_PANEL_COMPONENT_ID ||
    value.manifest_id !== RVELTE_DEV_EXPECTED_MANIFEST_ID ||
    value.source !== source ||
    value.runtime_route !== 'editor.diagnostics' ||
    typeof value.host_bridge_available !== 'boolean' ||
    !validateSnapshot(value.snapshot)
  ) {
    return null;
  }
  return value as RvelteDevMountMessage;
}

function validateSnapshot(value: unknown): value is RvelteDevPanelSnapshot {
  if (!isRecord(value)) {
    return false;
  }
  return (
    isBoundedCounter(value.revision, 999999) &&
    isBoundedCounter(value.errors, 999) &&
    isBoundedCounter(value.warnings, 999) &&
    isBoundedCounter(value.totalRows, 999) &&
    typeof value.runtimeLabel === 'string' &&
    value.runtimeLabel.length <= 96 &&
    typeof value.frameLabel === 'string' &&
    value.frameLabel.length <= 96 &&
    Array.isArray(value.rows) &&
    value.rows.length <= maxRows &&
    value.rows.every(validateRow)
  );
}

function validateRow(value: unknown): value is RvelteDevPanelRow {
  if (!isRecord(value)) {
    return false;
  }
  return (
    typeof value.id === 'string' &&
    value.id.length <= 64 &&
    (value.level === 'info' || value.level === 'warning' || value.level === 'error') &&
    (value.group === 'diagnostic' || value.group === 'runtime' || value.group === 'event' || value.group === 'command') &&
    typeof value.source === 'string' &&
    value.source.length <= 96 &&
    typeof value.message === 'string' &&
    value.message.length <= 180 &&
    typeof value.detail === 'string' &&
    value.detail.length <= 120
  );
}

function forwardRefreshRequested(
  mountMessage: RvelteDevMountMessage,
  event: unknown,
  onRefreshRequested: (revision: number) => void
): void {
  if (!isRecord(event) || !isBoundedCounter(event.revision, 999999)) {
    return;
  }
  const payload: RvelteDevPanelEvent = {
    schema_version: RVELTE_DEV_BRIDGE_SCHEMA_VERSION,
    kind: 'rvelte_dev_panel_refresh_requested',
    component_id: mountMessage.component_id,
    manifest_id: mountMessage.manifest_id,
    source: 'rvelte_diagnostics_log_panel',
    sequence: nextEventSequence++,
    revision: event.revision
  };
  emitHost('rvelte.dev_panel.refresh_requested', payload);
  const hostRequest = createRvelteHostCommandRequest('diagnostics.refresh', mountMessage, {
    current_revision: mountMessage.snapshot.revision,
    interaction_revision: payload.revision
  });
  if (hostRequest !== null) {
    emitHost('rvelte.host_bridge.request', hostRequest);
  }
  onRefreshRequested(payload.revision);
}

export function createRvelteHostCommandRequest(
  commandId: RvelteHostCommandId,
  mountMessage: RvelteDevMountMessage,
  payload:
    | {
        current_revision: number;
        max_rows: typeof maxRows;
      }
    | {
        current_revision: number;
        interaction_revision: number;
      }
): RvelteHostBridgeRequest | null {
  const payloadSizeBytes = encodedPayloadBytes(payload);
  const requestedCapability = commandId === 'diagnostics.refresh' ? 'RefreshDiagnostics' : 'ReadDiagnostics';
  const request: RvelteHostBridgeRequest = {
    schema_version: RVELTE_HOST_BRIDGE_SCHEMA_VERSION,
    kind: 'rvelte_host_command_request',
    component_id: mountMessage.component_id,
    manifest_id: mountMessage.manifest_id,
    source: 'rvelte_diagnostics_log_panel',
    command_id: commandId,
    correlation_id: `rvelte-dev-${nextHostCorrelationSequence++}`,
    request_revision: payload.current_revision,
    freshness_revision: mountMessage.snapshot.revision,
    payload_size_bytes: payloadSizeBytes,
    authorization_context: {
      authority: 'host_validated_required',
      requested_capability: requestedCapability
    },
    payload
  };
  return validateHostCommandRequest(request) ? request : null;
}

export function createRvelteDiagnosticsSnapshotRequest(
  mountMessage: RvelteDevMountMessage
): RvelteHostBridgeRequest | null {
  return createRvelteHostCommandRequest('diagnostics.snapshot.get', mountMessage, {
    current_revision: mountMessage.snapshot.revision,
    max_rows: maxRows
  });
}

function validateHostCommandRequest(value: unknown): value is RvelteHostBridgeRequest {
  if (!isRecord(value)) {
    return false;
  }
  const payload = value.payload;
  if (!isRecord(payload)) {
    return false;
  }
  const authorization = value.authorization_context;
  const commandId = value.command_id;
  if (
    value.schema_version !== RVELTE_HOST_BRIDGE_SCHEMA_VERSION ||
    value.kind !== 'rvelte_host_command_request' ||
    value.component_id !== RVELTE_DEV_PANEL_COMPONENT_ID ||
    value.manifest_id !== RVELTE_DEV_EXPECTED_MANIFEST_ID ||
    value.source !== 'rvelte_diagnostics_log_panel' ||
    (commandId !== 'diagnostics.snapshot.get' && commandId !== 'diagnostics.refresh') ||
    typeof value.correlation_id !== 'string' ||
    value.correlation_id.length > 48 ||
    !isBoundedCounter(value.request_revision, 999999) ||
    !isBoundedCounter(value.freshness_revision, 999999) ||
    !isBoundedCounter(value.payload_size_bytes, maxHostPayloadBytes) ||
    !isRecord(authorization) ||
    authorization.authority !== 'host_validated_required'
  ) {
    return false;
  }
  if (
    commandId === 'diagnostics.snapshot.get' &&
    authorization.requested_capability === 'ReadDiagnostics' &&
    isBoundedCounter(payload.current_revision, 999999) &&
    payload.max_rows === maxRows
  ) {
    return true;
  }
  return (
    commandId === 'diagnostics.refresh' &&
    authorization.requested_capability === 'RefreshDiagnostics' &&
    isBoundedCounter(payload.current_revision, 999999) &&
    isBoundedCounter(payload.interaction_revision, 999999)
  );
}

function diagnosticFromMountError(error: unknown): RvelteDevIslandDiagnostic {
  if (error instanceof Error && /wasm|fetch|instantiate/i.test(error.message)) {
    return rvelteDevDiagnostic('rvelte.wasm_missing');
  }
  return rvelteDevDiagnostic('rvelte.facade_incompatible');
}

function buildRows(diagnostics: Diagnostic[], events: EditorEvent[], current: EditorUiState): RvelteDevPanelRow[] {
  const editorDiagnostics = diagnostics.slice(-12).map((diagnostic, index) => ({
    id: compactText(`diagnostic:${diagnostic.code}:${index}`, 64),
    level: diagnostic.level,
    source: compactText(diagnostic.code, 72),
    message: compactText(diagnostic.message, 160),
    detail: compactDetail(diagnostic.target_path ?? diagnostic.hosted_instance_id ?? 'editor'),
    group: 'diagnostic' as const
  }));
  const runtimeDiagnostics =
    current.runtimeDiagnostics?.records.slice(-12).map((record) => ({
      id: compactText(`runtime:${record.sequence}`, 64),
      level: record.level,
      source: compactText(`${record.target}.${record.kind}`, 72),
      message: compactText(record.message, 160),
      detail: compactDetail(record.source ?? `frame ${record.frame_index ?? 'n/a'}`),
      group: 'runtime' as const
    })) ?? [];
  const eventRows = events.slice(-8).map((event, index) => ({
    id: compactText(`event:${event.timestamp}:${index}`, 64),
    level: event.level,
    source: compactText(event.command_id, 72),
    message: compactText(event.message, 160),
    detail: compactDetail(event.target_path ?? event.hosted_instance_id ?? event.timestamp),
    group: 'event' as const
  }));
  const commandRow = current.lastCommandResult
    ? [
        {
          id: 'command:last',
          level: current.lastCommandResult.ok ? ('info' as const) : ('error' as const),
          source: 'command.last_result',
          message: current.lastCommandResult.ok ? 'Last command completed.' : 'Last command failed.',
          detail: `${clampDiagnosticCount(current.lastCommandResult.diagnostics.length)} diagnostics`,
          group: 'command' as const
        }
      ]
    : [];
  return [...commandRow, ...runtimeDiagnostics.reverse(), ...editorDiagnostics.reverse(), ...eventRows.reverse()];
}

function compactDetail(value: string): string {
  const compact = compactText(value, 96);
  if (/[/\\]/.test(compact)) {
    const parts = compact.split(/[/\\]/).filter((part) => part.length > 0);
    const last = parts.at(-1) ?? 'path';
    return `path:${compactText(last, 48)}`;
  }
  return compact;
}

function compactText(value: string, maxLength: number): string {
  const compact = value.replace(/[|\r\n\t]+/g, ' ').replace(/\s{2,}/g, ' ').trim();
  const lower = compact.toLowerCase();
  if (
    lower.includes('password') ||
    lower.includes('token') ||
    lower.includes('ticket') ||
    lower.includes('session_id') ||
    lower.includes('cookie')
  ) {
    return '[redacted]';
  }
  return compact.slice(0, maxLength) || 'n/a';
}

function clampDiagnosticCount(value: number): number {
  if (!Number.isFinite(value) || value <= 0) {
    return 0;
  }
  return Math.min(999, Math.trunc(value));
}

function isBoundedCounter(value: unknown, max: number): value is number {
  return typeof value === 'number' && Number.isInteger(value) && value >= 0 && value <= max;
}

function encodedPayloadBytes(value: unknown): number {
  return new TextEncoder().encode(JSON.stringify(value ?? null)).length;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}
