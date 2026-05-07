<script lang="ts">
  import { onMount } from 'svelte';
  import type { EditorUiState } from '../types';
  import {
    RVELTE_DEV_ISLAND_ENABLED,
    RVELTE_DEV_PANEL_CSS_URL,
    createRvelteDevMountMessage,
    createRvelteDevPanelSnapshot,
    mountRvelteDevPanel,
    rvelteDevDiagnostic,
    updateRvelteDevPanelSnapshot,
    type RvelteDevIslandDiagnostic,
    type RvelteDevPanelInstance,
    type RvelteDevPanelSnapshot
  } from './bridge';

  export let state: EditorUiState;

  let islandRoot: HTMLDivElement;
  let islandStatus: HTMLParagraphElement;
  let mounted: RvelteDevPanelInstance | null = null;
  let diagnostics: RvelteDevIslandDiagnostic[] = [];
  let snapshot: RvelteDevPanelSnapshot = createRvelteDevPanelSnapshot(state);
  let mountedRevision = snapshot.revision;
  let refreshRevision = 0;
  let status: 'disabled' | 'loading' | 'mounted' | 'failed' = RVELTE_DEV_ISLAND_ENABLED ? 'loading' : 'disabled';

  $: snapshot = createRvelteDevPanelSnapshot(state);
  $: if (RVELTE_DEV_ISLAND_ENABLED && status === 'mounted' && mounted !== null) {
    try {
      if (updateRvelteDevPanelSnapshot(mounted, snapshot)) {
        mountedRevision = snapshot.revision;
      }
    } catch {
      diagnostics = [rvelteDevDiagnostic('rvelte.host_payload_invalid')];
      status = 'failed';
    }
  }
  $: statusText =
    status === 'mounted'
      ? 'mounted'
      : status === 'loading'
        ? 'loading'
        : status === 'failed'
          ? 'failed'
          : 'disabled';
  $: tone = status === 'mounted' ? 'good' : status === 'failed' ? 'bad' : 'neutral';

  onMount(() => {
    if (!RVELTE_DEV_ISLAND_ENABLED) {
      return;
    }

    let disposed = false;
    const removeStylesheet = ensureRvelteStylesheet();
    const message = createRvelteDevMountMessage(state);
    snapshot = message.snapshot;
    mountedRevision = snapshot.revision;

    void mountRvelteDevPanel(islandRoot, islandStatus, message, (revision) => {
      refreshRevision = revision;
    })
      .then((result) => {
        if (disposed) {
          result.instance?.destroy();
          return;
        }
        mounted = result.instance;
        diagnostics = result.diagnostics;
        status = result.instance ? 'mounted' : 'failed';
      })
      .catch(() => {
        if (!disposed) {
          diagnostics = [rvelteDevDiagnostic('rvelte.facade_incompatible')];
          status = 'failed';
        }
      });

    return () => {
      disposed = true;
      removeStylesheet();
      mounted?.destroy();
      mounted = null;
    };
  });

  function ensureRvelteStylesheet(): () => void {
    const id = 'fun-rvelte-dev-panel-style';
    if (document.getElementById(id)) {
      return () => {};
    }
    const link = document.createElement('link');
    link.id = id;
    link.rel = 'stylesheet';
    link.href = RVELTE_DEV_PANEL_CSS_URL;
    document.head.appendChild(link);
    return () => {
      link.remove();
    };
  }
</script>

{#if RVELTE_DEV_ISLAND_ENABLED}
  <section class="rvelte-dev-island" data-hit-region="diagnostics" aria-label="rvelte diagnostics dev panel">
    <header>
      <div>
        <span class="eyebrow">rvelte island</span>
        <h2>Diagnostics Log Panel</h2>
      </div>
      <div class={`status-chip ${tone}`}>
        <span>{statusText}</span>
        <strong>{snapshot.totalRows}</strong>
      </div>
    </header>

    <div class="rvelte-island-grid">
      <div bind:this={islandRoot} class="rvelte-panel-root"></div>
      <aside aria-label="rvelte bridge diagnostics">
        <div class="comparison-block">
          <span class="comparison-label">Svelte snapshot</span>
          <strong>{snapshot.errors} / {snapshot.warnings} / {snapshot.totalRows}</strong>
          <small>mounted rev {mountedRevision}{refreshRevision > 0 ? ` / refresh ${refreshRevision}` : ''}</small>
        </div>
        <p bind:this={islandStatus} class="rvelte-panel-status">loading</p>
        {#each diagnostics as diagnostic (diagnostic.code)}
          <article class={`rvelte-diagnostic ${diagnostic.severity}`}>
            <strong>{diagnostic.code}</strong>
            <span>{diagnostic.message}</span>
            <small>{diagnostic.detail}</small>
          </article>
        {:else}
          <article class="rvelte-diagnostic info">
            <strong>rvelte.bridge.ready</strong>
            <span>Typed diagnostics snapshot accepted.</span>
            <small>schema=fun.rvelte.dev_bridge.v1</small>
          </article>
        {/each}
      </aside>
    </div>
  </section>
{/if}

<style>
  .rvelte-dev-island {
    display: grid;
    gap: 14px;
    padding: 16px;
    border-bottom: 1px solid rgba(148, 163, 184, 0.2);
    background: #151a1f;
    color: #edf2f7;
  }

  .rvelte-dev-island header {
    display: flex;
    gap: 16px;
    align-items: center;
    justify-content: space-between;
  }

  .rvelte-dev-island h2 {
    margin: 2px 0 0;
    color: #ffffff;
    font-size: 1rem;
    font-weight: 700;
  }

  .rvelte-island-grid {
    display: grid;
    grid-template-columns: minmax(360px, 1.2fr) minmax(280px, 0.8fr);
    gap: 16px;
    align-items: stretch;
  }

  .rvelte-panel-root {
    min-height: 260px;
    padding: 14px;
    border: 1px solid rgba(148, 163, 184, 0.28);
    background: #111820;
    color: #e8f0f8;
  }

  .rvelte-island-grid aside {
    display: grid;
    gap: 8px;
    align-content: start;
    min-width: 0;
  }

  .comparison-block {
    display: grid;
    gap: 4px;
    padding: 10px;
    border: 1px solid rgba(148, 163, 184, 0.2);
    background: rgba(255, 255, 255, 0.035);
  }

  .comparison-label,
  .rvelte-panel-status {
    margin: 0;
    color: #a0aec0;
    font-size: 0.82rem;
  }

  .comparison-block strong {
    color: #ffffff;
    font-size: 1.05rem;
  }

  .comparison-block small {
    color: #a0aec0;
    overflow-wrap: anywhere;
  }

  .status-chip {
    display: inline-grid;
    grid-template-columns: auto auto;
    gap: 10px;
    align-items: center;
    min-width: 132px;
    padding: 8px 10px;
    border: 1px solid rgba(148, 163, 184, 0.28);
    font-size: 0.78rem;
    text-transform: uppercase;
  }

  .status-chip strong {
    justify-self: end;
    font-size: 1rem;
    color: #ffffff;
  }

  .status-chip.good {
    border-color: rgba(72, 187, 120, 0.45);
    color: #9ae6b4;
  }

  .status-chip.bad {
    border-color: rgba(252, 129, 129, 0.45);
    color: #feb2b2;
  }

  .status-chip.neutral {
    color: #cbd5e0;
  }

  .rvelte-diagnostic {
    display: grid;
    grid-template-columns: minmax(150px, 210px) 1fr;
    gap: 8px 12px;
    align-items: baseline;
    padding: 8px 10px;
    border: 1px solid rgba(148, 163, 184, 0.18);
    background: rgba(255, 255, 255, 0.035);
    font-size: 0.78rem;
  }

  .rvelte-diagnostic strong {
    color: #e2e8f0;
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 0.72rem;
    overflow-wrap: anywhere;
  }

  .rvelte-diagnostic span {
    color: #edf2f7;
  }

  .rvelte-diagnostic small {
    grid-column: 2;
    color: #a0aec0;
    overflow-wrap: anywhere;
  }

  .rvelte-diagnostic.error {
    border-color: rgba(252, 129, 129, 0.4);
  }

  .rvelte-diagnostic.warn {
    border-color: rgba(246, 173, 85, 0.38);
  }

  @media (max-width: 920px) {
    .rvelte-dev-island header,
    .rvelte-island-grid {
      grid-template-columns: 1fr;
    }

    .rvelte-island-grid {
      display: grid;
    }

    .rvelte-diagnostic {
      grid-template-columns: 1fr;
    }

    .rvelte-diagnostic small {
      grid-column: 1;
    }
  }
</style>
