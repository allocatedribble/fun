import { get, writable } from 'svelte/store';
import {
  getHostSnapshot,
  isHostRuntime,
  type FunHostSnapshotPayload,
  type FunHostState
} from '../commands';
import { subscribeHost } from '../host/commands';

export type HostStatePatchPayload =
  | FunHostSnapshotPayload
  | FunHostState
  | {
      snapshot?: FunHostSnapshotPayload['snapshot'];
      state?: FunHostState;
    };

export interface HostStoreState {
  ready: boolean;
  snapshot: FunHostSnapshotPayload | null;
  state: FunHostState | null;
  revision: number;
  error: string | null;
}

const initialHostStoreState: HostStoreState = {
  ready: false,
  snapshot: null,
  state: null,
  revision: 0,
  error: null
};

function createHostStore() {
  const store = writable<HostStoreState>(initialHostStoreState);
  let unlisten: (() => void) | null = null;
  let pendingInitialize: Promise<void> | null = null;

  function ensureSubscriptions(): void {
    if (unlisten || !isHostRuntime()) {
      return;
    }
    const unsubscribers = [
      subscribeHost<HostStatePatchPayload>('host.state.snapshot', applyPatch),
      subscribeHost<HostStatePatchPayload>('host.state.patch', applyPatch)
    ];
    unlisten = () => {
      unsubscribers.forEach((unsubscribe) => unsubscribe());
      unlisten = null;
    };
  }

  function applyPatch(payload: HostStatePatchPayload): void {
    const snapshot = snapshotFromPayload(payload);
    const state = snapshot?.snapshot.state ?? hostStateFromPayload(payload);
    if (!state) {
      return;
    }
    store.update((current) => ({
      ready: true,
      snapshot: snapshot ?? current.snapshot,
      state,
      revision: current.revision + 1,
      error: null
    }));
  }

  async function initialize(): Promise<void> {
    if (!isHostRuntime()) {
      return;
    }
    ensureSubscriptions();
    if (pendingInitialize) {
      return pendingInitialize;
    }
    pendingInitialize = getHostSnapshot()
      .then((snapshot) => applyPatch(snapshot))
      .catch((error) => {
        store.update((current) => ({
          ...current,
          error: error instanceof Error ? error.message : String(error)
        }));
        throw error;
      })
      .finally(() => {
        pendingInitialize = null;
      });
    return pendingInitialize;
  }

  async function refresh(): Promise<void> {
    if (!isHostRuntime()) {
      return;
    }
    ensureSubscriptions();
    applyPatch(await getHostSnapshot());
  }

  function reset(): void {
    unlisten?.();
    store.set(initialHostStoreState);
  }

  return {
    subscribe: store.subscribe,
    initialize,
    refresh,
    applyPatch,
    reset,
    current: () => get(store)
  };
}

export const hostStore = createHostStore();

export function snapshotFromPayload(payload: HostStatePatchPayload): FunHostSnapshotPayload | null {
  if (!payload || typeof payload !== 'object') {
    return null;
  }
  const record = payload as {
    snapshot?: FunHostSnapshotPayload['snapshot'];
    command_catalog?: FunHostSnapshotPayload['command_catalog'];
    editor_command_catalog?: FunHostSnapshotPayload['editor_command_catalog'];
  };
  if (record.snapshot?.state) {
    return {
      snapshot: record.snapshot,
      command_catalog: record.command_catalog ?? [],
      editor_command_catalog: record.editor_command_catalog ?? []
    };
  }
  return null;
}

export function hostStateFromPayload(payload: HostStatePatchPayload): FunHostState | null {
  if (!payload || typeof payload !== 'object') {
    return null;
  }
  const record = payload as {
    snapshot?: FunHostSnapshotPayload['snapshot'];
    state?: FunHostState;
    mode?: FunHostState['mode'];
  };
  if (record.snapshot?.state) {
    return record.snapshot.state;
  }
  if (record.state) {
    return record.state;
  }
  return record.mode ? (record as FunHostState) : null;
}
