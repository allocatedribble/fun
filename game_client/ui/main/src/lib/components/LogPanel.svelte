<script lang="ts">
  import type { Diagnostic, EditorEvent, EditorUiState } from '../types';
  import StatusPill from './StatusPill.svelte';

  export let state: EditorUiState;

  type LogRow = {
    id: string;
    level: 'info' | 'warning' | 'error';
    source: string;
    message: string;
    detail: string;
    group: 'diagnostic' | 'runtime' | 'event' | 'command';
  };

  $: runtime = state.runtimeStatus ?? state.status?.runtime_status ?? null;
  $: auth = state.status?.auth_session ?? null;
  $: commandJson = state.lastCommandResult
    ? JSON.stringify(state.lastCommandResult, null, 2)
    : JSON.stringify({ ok: true, value: null, diagnostics: [], note: 'No command result captured yet.' }, null, 2);
  $: rows = buildRows(state.diagnostics, state.events, state);
  $: errors = rows.filter((row) => row.level === 'error').length;
  $: warnings = rows.filter((row) => row.level === 'warning').length;
  $: frame = state.runtimeDiagnostics?.latest_frame_profile;

  function buildRows(diagnostics: Diagnostic[], events: EditorEvent[], current: EditorUiState): LogRow[] {
    const editorDiagnostics = diagnostics.map((diagnostic, index) => ({
      id: `diagnostic:${diagnostic.code}:${index}`,
      level: diagnostic.level,
      source: diagnostic.code,
      message: diagnostic.message,
      detail: diagnostic.target_path ?? diagnostic.hosted_instance_id ?? 'editor',
      group: 'diagnostic' as const
    }));
    const runtimeDiagnostics =
      current.runtimeDiagnostics?.records.slice(-160).map((record) => ({
        id: `runtime:${record.sequence}`,
        level: record.level,
        source: `${record.target}.${record.kind}`,
        message: record.message,
        detail: record.source ?? `frame ${record.frame_index ?? 'n/a'}`,
        group: 'runtime' as const
      })) ?? [];
    const eventRows = events.slice(-120).map((event, index) => ({
      id: `event:${event.timestamp}:${index}`,
      level: event.level,
      source: event.command_id,
      message: event.message,
      detail: event.target_path ?? event.hosted_instance_id ?? event.timestamp,
      group: 'event' as const
    }));
    const commandRow = current.lastCommandResult
      ? [
          {
            id: 'command:last',
            level: current.lastCommandResult.ok ? ('info' as const) : ('error' as const),
            source: 'command.last_result',
            message: current.lastCommandResult.ok ? 'Last command completed.' : 'Last command failed.',
            detail: `${current.lastCommandResult.diagnostics.length} diagnostics`,
            group: 'command' as const
          }
        ]
      : [];
    return [...commandRow, ...runtimeDiagnostics.reverse(), ...editorDiagnostics.reverse(), ...eventRows.reverse()];
  }
</script>

<section class="log-panel panel" aria-label="Log">
  <header class="log-panel-header">
    <div>
      <span class="eyebrow">Log</span>
      <h2>{errors} errors / {warnings} warnings / {rows.length} rows</h2>
    </div>
    <div class="runtime-status-grid compact">
      <StatusPill label="preview" value={state.previewRenderStatus.status} tone={state.previewRenderStatus.status === 'ready' ? 'good' : state.previewRenderStatus.status === 'failed' ? 'bad' : 'neutral'} />
      <StatusPill label="client" value={runtime?.client_process.state ?? 'unknown'} tone={runtime?.client_process.state === 'running' ? 'good' : 'neutral'} />
      <StatusPill label="server" value={runtime?.server_process.state ?? 'unknown'} tone={runtime?.server_process.state === 'running' ? 'good' : 'neutral'} />
      <StatusPill label="auth" value={auth?.token_redacted ? 'redacted' : 'none'} tone={auth?.token_redacted ? 'good' : 'neutral'} />
    </div>
  </header>

  <div class="log-panel-body">
    <section class="log-stream" aria-label="Combined editor log stream">
      <div class="log-stream-head">
        <strong>Combined stream</strong>
        <span>diagnostics, runtime trace, events, command results</span>
      </div>
      <div class="log-list merged-log-list">
        {#each rows as row (row.id)}
          <article class={`event-row ${row.level}`}>
            <span class="mono">{row.group}</span>
            <strong>{row.source}</strong>
            <p>{row.message}</p>
            <small>{row.detail}</small>
          </article>
        {:else}
          <p class="muted">No log rows yet.</p>
        {/each}
      </div>
    </section>

    <aside class="log-inspector" aria-label="Log context">
      <section>
        <h3>Runtime</h3>
        <div class="property-list">
          <article><strong>Control address</strong><span class="mono">{auth?.control_addr ?? '127.0.0.1:0'}</span></article>
          <article><strong>Session</strong><span class="mono">{auth?.session_id ?? 'none'}</span></article>
          <article><strong>Dropped packets</strong><span>{state.runtimeDiagnostics?.dropped_packets ?? 0}</span></article>
        </div>
      </section>

      <section>
        <h3>Frame profile</h3>
        <div class="property-list">
          <article><strong>Frame</strong><span>{frame?.frame ?? 'none'}</span></article>
          <article><strong>Main</strong><span>{frame ? `${Math.round(frame.main_thread_ns / 1000)} us` : 'n/a'}</span></article>
          <article><strong>Render</strong><span>{frame ? `${Math.round(frame.render_thread_ns / 1000)} us` : 'n/a'}</span></article>
          <article><strong>GPU</strong><span>{frame ? `${Math.round(frame.gpu_ns / 1000)} us` : 'n/a'}</span></article>
        </div>
      </section>

      <section>
        <h3>Last command</h3>
        <pre class="command-result-pre">{commandJson}</pre>
      </section>
    </aside>
  </div>
</section>
