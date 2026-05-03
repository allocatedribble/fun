<script lang="ts">
  import type { GraphData, GraphValidationIssue } from '../../graph/types';

  export let graph: GraphData;
  export let validationIssues: GraphValidationIssue[];
  export let expanded = false;
  export let savedAt: string | null = null;
  export let message: string | null = null;
  export let shaderPath = '';
  export let canUndo = false;
  export let canRedo = false;
  export let onToggle: () => void;
  export let onValidate: () => void;
  export let onSaveNative: () => void | Promise<void>;
  export let onRefreshShaders: () => void | Promise<void>;
  export let onUndo: () => void;
  export let onRedo: () => void;
  export let onCopyWgsl: () => void | Promise<void>;
  export let onPasteWgsl: () => void | Promise<void>;

  $: errors = validationIssues.filter((issue) => issue.level === 'error').length;
  $: warnings = validationIssues.filter((issue) => issue.level === 'warning').length;
  $: statusTone = errors > 0 ? 'bad' : warnings > 0 ? 'warn' : 'good';
  $: statusText = errors > 0 ? `${errors} errors` : warnings > 0 ? `${warnings} warnings` : 'valid';
</script>

<section class:expanded class="graph-status-strip">
  <div class="graph-status-main">
    <button class="button is-small" type="button" on:click={onToggle} title={expanded ? 'Collapse status' : 'Expand status'}>
      {expanded ? 'Hide' : 'Status'}
    </button>
    <span class={`status-pill ${statusTone}`}><strong>{statusText}</strong></span>
    <span class="mini-tag">{graph.nodes.length} nodes</span>
    <span class="mini-tag">{graph.connections.length} wires</span>
    <span class="mini-tag">zoom {Math.round(graph.viewState.zoom * 100)}%</span>
    {#if shaderPath}
      <span class="mini-tag">{shaderPath}</span>
    {/if}
    {#if savedAt}
      <span class="mini-tag">saved {savedAt}</span>
    {/if}
    {#if message}
      <span class="graph-status-message">{message}</span>
    {/if}
    <span class="graph-status-spacer"></span>
    <button class="button is-small" type="button" on:click={onUndo} disabled={!canUndo}>Undo</button>
    <button class="button is-small" type="button" on:click={onRedo} disabled={!canRedo}>Redo</button>
    <button class="button is-small" type="button" on:click={onValidate}>Validate</button>
    <button class="button is-small" type="button" on:click={onSaveNative}>Save WGSL</button>
    <button class="button is-small" type="button" on:click={onRefreshShaders}>Refresh Files</button>
    <button class="button is-small" type="button" on:click={onCopyWgsl}>Copy WGSL</button>
    <button class="button is-small" type="button" on:click={onPasteWgsl}>Paste WGSL</button>
  </div>

  {#if expanded}
    <div class="graph-status-details">
      {#each validationIssues.slice(0, 8) as issue (issue.id)}
        <button class={`graph-validation-row ${issue.level}`} type="button">
          <strong>{issue.level}</strong>
          <span>{issue.message}</span>
        </button>
      {:else}
        <p class="muted">No validation issues.</p>
      {/each}
    </div>
  {/if}
</section>
