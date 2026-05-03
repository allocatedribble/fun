import './bridge';
import type { HostEventHandler } from './bridge';

export async function invokeHost<T>(command: string, payload?: unknown): Promise<T> {
  return window.fun.request(command, payload ?? null);
}

export function subscribeHost<T>(event: string, handler: HostEventHandler<T>): () => void {
  return window.fun.subscribe(event, handler);
}

export function emitHost(event: string, payload?: unknown): void {
  window.fun.emit(event, payload ?? null);
}

export function isHostRuntime(): boolean {
  return typeof window !== 'undefined' && (typeof window.funHost?.postMessage === 'function' || typeof window.cefQuery === 'function');
}
