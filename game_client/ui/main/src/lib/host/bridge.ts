type HostRequestResolver = {
  command: string;
  resolve: (value: unknown) => void;
  reject: (reason?: unknown) => void;
};

export type HostEventHandler<T = unknown> = (payload: T) => void;

export type FunHostBridge = {
  request<T>(command: string, payload: unknown): Promise<T>;
  subscribe<T>(event: string, handler: HostEventHandler<T>): () => void;
  emit(event: string, payload?: unknown): void;
  receiveFromHost(envelope: unknown): void;
};

declare global {
  interface Window {
    cefQuery?: (request: {
      request: string;
      persistent?: boolean;
      onSuccess?: (response: string) => void;
      onFailure?: (errorCode: number, errorMessage: string) => void;
    }) => void;
    funHost?: {
      postMessage: (envelope: unknown) => void;
    };
    fun: FunHostBridge;
  }
}

const protocolVersion = 1;
const schemaRevision = 1;
let nextRequestId = 1;
let nextSequence = 1;
const pendingRequests = new Map<number, HostRequestResolver>();
const subscribers = new Map<string, Set<HostEventHandler>>();
const textEncoder = new TextEncoder();
const textDecoder = new TextDecoder();
const maxHostPayloadBytes = 64 * 1024;

function encodeJsonBytes(value: unknown): number[] {
  return Array.from(textEncoder.encode(JSON.stringify(value ?? null)));
}

function decodeJsonBytes(bytes: unknown): unknown {
  if (Array.isArray(bytes)) {
    return JSON.parse(textDecoder.decode(Uint8Array.from(bytes)));
  }
  if (typeof bytes === 'string') {
    return JSON.parse(bytes);
  }
  if (bytes instanceof Uint8Array) {
    return JSON.parse(textDecoder.decode(bytes));
  }
  return bytes ?? null;
}

function commandEnvelope(command: string, payload: unknown, requestId: number): unknown {
  const payloadBytes = encodeJsonBytes(payload);
  return {
    protocol_version: protocolVersion,
    schema_revision: schemaRevision,
    channel: 'control',
    kind: 'request',
    request_id: requestId,
    sequence: nextSequence++,
    payload: {
      Control: {
        payload: {
          HostCommand: {
            request: {
              command_id: command,
              request_id: requestId,
              payload: payloadBytes,
              capability: capabilityForCommand(command),
              size_budget: { max_bytes: maxHostPayloadBytes }
            }
          }
        }
      }
    }
  };
}

function hostBridgeUnavailable(command: string): Error {
  return new Error(`Fun host bridge unavailable for ${command}.`);
}

function postEnvelope(envelope: unknown, requestId: number, command: string): void {
  if (window.funHost?.postMessage) {
    window.funHost.postMessage(envelope);
    return;
  }

  if (window.cefQuery) {
    window.cefQuery({
      request: JSON.stringify(envelope),
      persistent: false,
      onSuccess: (response) => {
        if (!response) {
          return;
        }
        window.fun.receiveFromHost(JSON.parse(response));
      },
      onFailure: (_errorCode, errorMessage) => {
        const resolver = pendingRequests.get(requestId);
        pendingRequests.delete(requestId);
        resolver?.reject(new Error(errorMessage || `Fun host rejected ${command}.`));
      }
    });
    return;
  }

  throw hostBridgeUnavailable(command);
}

function resolveHostCommandResult(envelope: Record<string, unknown>): void {
  const requestId = Number(envelope.request_id ?? 0);
  const resolver = pendingRequests.get(requestId);
  if (!resolver) {
    return;
  }
  pendingRequests.delete(requestId);

  const controlPayload = extractControlPayload(envelope);
  const result = extractHostCommandResult(controlPayload);
  if (!result) {
    resolver.reject(new Error(`Malformed Fun host response for ${resolver.command}.`));
    return;
  }

  const commandId = commandIdFromWire(result.command_id ?? resolver.command);
  if (commandId !== resolver.command) {
    resolver.reject(new Error(`Fun host response command mismatch: ${commandId}.`));
    return;
  }

  const typedResponse = result.response;
  if (typeof typedResponse === 'object' && typedResponse !== null) {
    resolveTypedHostCommandResponse(resolver, commandId, typedResponse as Record<string, unknown>);
    return;
  }

  const status = String(result.status ?? 'error');
  if (status === 'ok') {
    resolver.resolve(decodeJsonBytes(result.payload_json));
  } else {
    const errorCode = result.error_code ? ` (${String(result.error_code)})` : '';
    const decoded = decodeJsonBytes(result.payload_json);
    const message =
      typeof decoded === 'object' && decoded !== null && 'message' in decoded
        ? String((decoded as { message?: unknown }).message)
        : `Fun host command failed: ${commandId}${errorCode}.`;
    resolver.reject(new Error(message));
  }
}

function extractControlPayload(envelope: Record<string, unknown>): unknown {
  const payload = envelope.payload;
  if (typeof payload !== 'object' || payload === null) {
    return null;
  }
  const payloadRecord = payload as Record<string, unknown>;
  const control = payloadRecord.Control ?? payloadRecord.control;
  if (typeof control !== 'object' || control === null) {
    return payload;
  }
  return (control as Record<string, unknown>).payload ?? control;
}

function extractHostCommandResult(payload: unknown): Record<string, unknown> | null {
  if (typeof payload !== 'object' || payload === null) {
    return null;
  }
  const record = payload as Record<string, unknown>;
  const direct = record.HostCommandResult ?? record.host_command_result;
  if (typeof direct === 'object' && direct !== null) {
    return direct as Record<string, unknown>;
  }
  return 'command_id' in record && 'payload_json' in record ? record : null;
}

function extractHostEvent(payload: unknown): { event: string; payload: unknown } | null {
  if (typeof payload !== 'object' || payload === null) {
    return null;
  }
  const record = payload as Record<string, unknown>;
  const direct = record.HostEvent ?? record.host_event;
  if (typeof direct === 'object' && direct !== null) {
    const eventRecord = direct as Record<string, unknown>;
    const event = typeof eventRecord.event === 'string' ? eventRecord.event : '';
    return event ? { event, payload: decodeJsonBytes(eventRecord.payload) } : null;
  }
  return null;
}

function commandIdFromWire(value: unknown): string {
  if (typeof value === 'string') {
    return value;
  }
  if (typeof value === 'object' && value !== null && 'value' in value) {
    return String((value as { value?: unknown }).value ?? '');
  }
  return String(value ?? '');
}

function resolveTypedHostCommandResponse(
  resolver: HostRequestResolver,
  commandId: string,
  response: Record<string, unknown>
): void {
  const ok = response.Ok ?? response.ok;
  if (typeof ok === 'object' && ok !== null) {
    resolver.resolve(decodeJsonBytes((ok as { payload?: unknown }).payload));
    return;
  }

  const rejected = response.Rejected ?? response.rejected;
  if (typeof rejected === 'object' && rejected !== null) {
    const reason = (rejected as { reason?: unknown }).reason ?? 'rejected';
    resolver.reject(new Error(`Fun host rejected ${commandId}: ${String(reason)}.`));
    return;
  }

  const failed = response.Failed ?? response.failed;
  if (typeof failed === 'object' && failed !== null) {
    const error = (failed as { error?: unknown }).error;
    const message =
      typeof error === 'object' && error !== null && 'message' in error
        ? String((error as { message?: unknown }).message)
        : `Fun host command failed: ${commandId}.`;
    resolver.reject(new Error(message));
    return;
  }

  resolver.reject(new Error(`Malformed Fun host response for ${resolver.command}.`));
}

function capabilityForCommand(command: string): string {
  if (command === 'games.join') {
    return 'JoinGame';
  }
  if (
    command === 'project.open' ||
    command === 'project.edit.open' ||
    command === 'editor.activate' ||
    command === 'editor.deactivate' ||
    command === 'editor.overlay.toggle'
  ) {
    return 'EditProject';
  }
  if (command.startsWith('entity.transform.')) {
    return 'MutateEntities';
  }
  if (command.startsWith('entity') || command.startsWith('live_entity')) {
    return 'ReadEntities';
  }
  if (command.startsWith('material.shader.save')) {
    return 'MaterialShaderWrite';
  }
  if (command.startsWith('material.shader.')) {
    return 'MaterialShaderRead';
  }
  if (command.includes('diagnostics')) {
    return 'ReadDiagnostics';
  }
  if (
    command.startsWith('account.login') ||
    command.startsWith('account.register') ||
    command.startsWith('account.logout') ||
    command.startsWith('auth.ticket') ||
    command.startsWith('auth.account.ticket') ||
    command === 'auth.logout'
  ) {
    return 'RequestBackendTicket';
  }
  if (
    command.startsWith('runtime.') ||
    command.startsWith('viewport.') ||
    command.startsWith('preview.') ||
    command === 'host.commandbar.execute' ||
    command === 'bevy.demo.launch'
  ) {
    return 'ControlRuntime';
  }
  if (command.startsWith('window.')) {
    return 'UseDevTools';
  }
  if (command.startsWith('project') || command.startsWith('projects')) {
    return 'OpenProject';
  }
  return 'ReadLauncher';
}

function dispatchHostEvent(event: string, payload: unknown): void {
  const handlers = subscribers.get(event);
  if (!handlers) {
    return;
  }
  for (const handler of handlers) {
    handler(payload);
  }
}

function receiveFromHost(envelope: unknown): void {
  if (typeof envelope !== 'object' || envelope === null) {
    return;
  }
  const record = envelope as Record<string, unknown>;
  if (record.kind === 'response' || record.kind === 'error') {
    resolveHostCommandResult(record);
    return;
  }
  if (record.kind === 'event') {
    const hostEvent = extractHostEvent(extractControlPayload(record));
    if (hostEvent) {
      dispatchHostEvent(hostEvent.event, hostEvent.payload);
      return;
    }
  }
  if (record.kind === 'patch') {
    dispatchHostEvent('host.state.patch', record.payload ?? null);
    return;
  }

  const event = record.event ?? record.type ?? record.name;
  if (typeof event === 'string') {
    dispatchHostEvent(event, record.payload ?? null);
  }
}

function request<T>(command: string, payload: unknown): Promise<T> {
  const trimmedCommand = command.trim();
  if (!trimmedCommand) {
    return Promise.reject(new Error('Fun host command id cannot be empty.'));
  }
  const requestId = nextRequestId++;
  const envelope = commandEnvelope(trimmedCommand, payload, requestId);
  return new Promise<T>((resolve, reject) => {
    pendingRequests.set(requestId, {
      command: trimmedCommand,
      resolve: (value) => resolve(value as T),
      reject
    });
    try {
      postEnvelope(envelope, requestId, trimmedCommand);
    } catch (error) {
      pendingRequests.delete(requestId);
      reject(error);
    }
  });
}

function subscribe<T>(event: string, handler: HostEventHandler<T>): () => void {
  const handlers = subscribers.get(event) ?? new Set<HostEventHandler>();
  handlers.add(handler as HostEventHandler);
  subscribers.set(event, handlers);
  return () => {
    handlers.delete(handler as HostEventHandler);
    if (handlers.size === 0) {
      subscribers.delete(event);
    }
  };
}

function emit(event: string, payload: unknown = null): void {
  const envelope = { kind: 'event', event, payload, sequence: nextSequence++ };
  dispatchHostEvent(event, payload);
  if (window.funHost?.postMessage) {
    window.funHost.postMessage(envelope);
    return;
  }
  if (window.cefQuery) {
    window.cefQuery({
      request: JSON.stringify(envelope),
      persistent: false
    });
  }
}

if (typeof window !== 'undefined' && !window.fun) {
  window.fun = {
    request,
    subscribe,
    emit,
    receiveFromHost
  };
}

export {};
