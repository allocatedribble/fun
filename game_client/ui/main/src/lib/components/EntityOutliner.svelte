<script lang="ts">
  import { onMount } from 'svelte';
  import type { DomainFilter, EditorUiState, EntityRowSummary, SourceFilter } from '../types';
  import DomainBadge from './DomainBadge.svelte';

  export let state: EditorUiState;
  export let onRange: (start: number, end: number) => void | Promise<void>;
  export let onSelect: (row: EntityRowSummary) => void | Promise<void>;
  export let onSearch: (query: string) => void;
  export let onDomain: (domain: DomainFilter) => void;
  export let onSource: (source: SourceFilter) => void;

  const rowHeight = 38;
  const domains: { value: DomainFilter; label: string }[] = [
    { value: 'all', label: 'All' },
    { value: 'client', label: 'Client' },
    { value: 'server', label: 'Server' },
    { value: 'shared', label: 'Shared' }
  ];
  const sources: { value: SourceFilter; label: string }[] = [
    { value: 'live', label: 'Live' },
    { value: 'bsn', label: 'Fun Scene' },
    { value: 'both', label: 'Both' }
  ];

  let scroller: HTMLDivElement | undefined;
  let scrollTop = 0;
  let viewportHeight = 460;
  let lastRangeKey = '';

  $: totalRows = state.entities.totalRows;
  $: overscan = state.entities.overscanRows;
  $: startIndex = Math.max(0, Math.floor(scrollTop / rowHeight) - overscan);
  $: visibleCount = Math.ceil(viewportHeight / rowHeight) + overscan * 2;
  $: endIndex = Math.min(totalRows, startIndex + visibleCount);
  $: visibleIndexes = Array.from({ length: Math.max(0, endIndex - startIndex) }, (_, index) => startIndex + index);
  $: requestVisibleRange(startIndex, endIndex);

  onMount(() => {
    measure();
    requestVisibleRange(0, visibleCount);
  });

  function handleScroll(): void {
    if (!scroller) {
      return;
    }
    scrollTop = scroller.scrollTop;
    measure();
  }

  function measure(): void {
    if (scroller) {
      viewportHeight = scroller.clientHeight || viewportHeight;
    }
  }

  function requestVisibleRange(start: number, end: number): void {
    const key = `${state.entities.cursor?.cursor ?? 'none'}:${start}:${end}`;
    if (key === lastRangeKey || end <= start) {
      return;
    }
    lastRangeKey = key;
    void onRange(start, end);
  }
</script>

<section class="outliner-panel">
  <div class="panel-heading-tight">
    <span class="eyebrow">Entity outliner</span>
    <h2>{totalRows.toLocaleString()} rows</h2>
  </div>

  <div class="field">
    <div class="control">
      <input
        class="input is-small"
        type="search"
        placeholder="name, component, source path, authority"
        value={state.entities.query}
        on:input={(event) => onSearch(event.currentTarget.value)}
      />
    </div>
  </div>

  <div class="segmented-row">
    <div class="buttons has-addons domain-filter">
      {#each domains as option}
        <button
          class:active={state.entities.domain === option.value}
          class="button is-small"
          type="button"
          on:click={() => onDomain(option.value)}
        >
          {option.label}
        </button>
      {/each}
    </div>

    <div class="buttons has-addons domain-filter">
      {#each sources as option}
        <button
          class:active={state.entities.source === option.value}
          class="button is-small"
          type="button"
          on:click={() => onSource(option.value)}
        >
          {option.label}
        </button>
      {/each}
    </div>
  </div>

  <div class="outliner-meta">
    <span>{state.entities.pageSize} page</span>
    <span>{state.entities.overscanRows} overscan</span>
    <span>{state.entities.loading ? 'loading' : 'ready'}</span>
  </div>

  <div class="virtual-list" bind:this={scroller} on:scroll={handleScroll}>
    <div class="virtual-spacer" style={`height: ${totalRows * rowHeight}px;`}>
      {#each visibleIndexes as index (index)}
        {@const row = state.entities.rowsByIndex[index]}
        {#if row}
          <button
            class:selected={state.entities.selectedId === row.id}
            class="entity-row"
            style={`transform: translateY(${index * rowHeight}px); height: ${rowHeight}px;`}
            type="button"
            on:click={() => onSelect(row)}
          >
            <span class="entity-main">
              <span class="entity-name">{row.name}</span>
              <span class="entity-id mono">{row.id}</span>
            </span>
            <span class="entity-badges">
              <DomainBadge domain={row.domain} />
              <span class="mini-tag">
                {row.source_kind === 'server_live' ? 'server live' : row.source_kind === 'client_live' ? 'client live' : row.live_snapshot ? 'live' : 'static'}
              </span>
              <span class="mini-tag">{row.component_count}</span>
              <span class="mini-tag">{row.source_kind === 'source_bsn' ? 'Fun' : 'RT'}</span>
            </span>
          </button>
        {:else}
          <div class="entity-row placeholder" style={`transform: translateY(${index * rowHeight}px); height: ${rowHeight}px;`}>
            <span>loading {index}</span>
          </div>
        {/if}
      {/each}
    </div>
  </div>
</section>
