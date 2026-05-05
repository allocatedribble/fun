<script lang="ts">
  import { onMount } from 'svelte';
  import type {
    DomainFilter,
    EditorUiState,
    EntityRowSummary,
    OverviewSelection,
    SourceFilter
  } from '../types';
  import DomainBadge from './DomainBadge.svelte';

  export let state: EditorUiState;
  export let onRange: (start: number, end: number) => void | Promise<void>;
  export let onSelectEntity: (row: EntityRowSummary) => void | Promise<void>;
  export let onSearch: (query: string) => void;
  export let onDomain: (domain: DomainFilter) => void;
  export let onSource: (source: SourceFilter) => void;
  export let onSelectOverview: (selection: OverviewSelection) => void;

  const rowHeight = 38;
  const domains: { value: DomainFilter; label: string }[] = [
    { value: 'all', label: 'All' },
    { value: 'client', label: 'Client' },
    { value: 'server', label: 'Server' },
    { value: 'shared', label: 'Shared' }
  ];
  const sources: { value: SourceFilter; label: string }[] = [
    { value: 'both', label: 'Both' },
    { value: 'live', label: 'Live' },
    { value: 'bsn', label: 'Fun Scene' }
  ];

  let scroller: HTMLDivElement | undefined;
  let scrollTop = 0;
  let viewportHeight = 460;
  let lastRangeKey = '';

  $: query = state.overview.query || state.entities.query;
  $: projectRows = projectOverviewRows(state).filter((row) => matches(row, query));
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

  function matches(row: OverviewSelection, value: string): boolean {
    const normalized = value.trim().toLowerCase();
    if (!normalized) {
      return true;
    }
    const haystack = [
      row.kind,
      row.title,
      row.subtitle,
      row.path,
      ...Object.entries(row.metadata).map(([key, data]) => `${key} ${data ?? ''}`)
    ]
      .filter(Boolean)
      .join(' ')
      .toLowerCase();
    return normalized.split(/\s+/).every((token) => haystack.includes(token));
  }

  function selectProjectRow(row: OverviewSelection): void {
    onSelectOverview(row);
  }

  function selectEntity(row: EntityRowSummary): void {
    void onSelectEntity(row);
  }

  function isSelected(id: string): boolean {
    return state.overview.selectedItem?.id === id || state.entities.selectedId === id;
  }

  function projectOverviewRows(current: EditorUiState): OverviewSelection[] {
    const project = current.project;
    if (!project) {
      return [];
    }
    const rows: OverviewSelection[] = [
      {
        kind: 'project',
        id: project.id,
        title: project.display_name,
        subtitle: 'Project root',
        path: project.root_path,
        scope: { type: 'current_project', projectId: project.id },
        metadata: {
          crates: project.crates.length,
          targets: project.targets.length,
          assets: project.asset_roots.length,
          scenes: project.bsn_index.detected_scene_count
        }
      },
      ...project.crates.map((projectCrate) => ({
        kind: 'crate' as const,
        id: `crate:${projectCrate.name}`,
        title: projectCrate.name,
        subtitle: projectCrate.role,
        path: projectCrate.manifest_path,
        scope: { type: 'current_project' as const, projectId: project.id },
        metadata: {
          domain: projectCrate.domain ?? 'none',
          targets: projectCrate.target_names.join(', ') || 'none',
          role: projectCrate.role
        }
      })),
      ...project.targets.map((target) => ({
        kind: 'target' as const,
        id: `target:${target}`,
        title: target,
        subtitle: 'Cargo target',
        path: target,
        scope: { type: 'current_project' as const, projectId: project.id },
        metadata: { target }
      })),
      ...project.asset_roots.map((root) => ({
        kind: 'directory' as const,
        id: `asset-root:${root}`,
        title: root.split(/[\\/]/).filter(Boolean).at(-1) ?? root,
        subtitle: 'Asset root',
        path: root,
        scope: { type: 'current_project' as const, projectId: project.id },
        metadata: { role: 'asset root' }
      })),
      ...project.bsn_roots.map((root) => ({
        kind: 'directory' as const,
        id: `bsn-root:${root}`,
        title: root.split(/[\\/]/).filter(Boolean).at(-1) ?? root,
        subtitle: 'Fun scene root',
        path: root,
        scope: { type: 'current_project' as const, projectId: project.id },
        metadata: { role: 'bsn root' }
      })),
      ...project.bsn_index.records.map((scene) => ({
        kind: 'scene' as const,
        id: `scene:${scene.id}`,
        title: scene.scene_function_name ?? scene.detected_names[0] ?? scene.invocation_kind,
        subtitle: `${scene.domain ?? 'shared'} scene`,
        path: scene.file_path,
        scope: { type: 'current_project' as const, projectId: project.id },
        metadata: {
          invocation: scene.invocation_kind,
          domain: scene.domain ?? 'shared',
          components: scene.component_type_tokens.length,
          names: scene.detected_names.join(', ') || 'none'
        }
      })),
      ...project.diagnostics.map((diagnostic, index) => ({
        kind: 'diagnostic' as const,
        id: `project-diagnostic:${diagnostic.code}:${index}`,
        title: diagnostic.code,
        subtitle: diagnostic.message,
        path: diagnostic.target_path ?? undefined,
        scope: { type: 'current_project' as const, projectId: project.id },
        metadata: {
          level: diagnostic.level,
          hostedInstance: diagnostic.hosted_instance_id ?? 'none'
        }
      }))
    ];
    return rows;
  }
</script>

<section class="overview-panel panel" aria-label="Overview">
  <header class="overview-header">
    <div>
      <span class="eyebrow">Overview</span>
      <h2>{state.project?.display_name ?? 'Open a project'}</h2>
    </div>
    <div class="overview-stats">
      <span class="tag is-small">{projectRows.length} project items</span>
      <span class="tag is-small">{totalRows.toLocaleString()} entities</span>
      <span class="tag is-small">{state.entities.loading ? 'loading' : 'ready'}</span>
    </div>
  </header>

  <div class="overview-search-row">
    <input
      class="input is-small"
      type="search"
      placeholder="Search project, directories, assets, scenes, entities, diagnostics"
      value={query}
      on:input={(event) => onSearch(event.currentTarget.value)}
    />
    <div class="buttons has-addons overview-filter-group">
      {#each domains as option}
        <button class:active={state.entities.domain === option.value} class="button is-small" type="button" on:click={() => onDomain(option.value)}>
          {option.label}
        </button>
      {/each}
    </div>
    <div class="buttons has-addons overview-filter-group">
      {#each sources as option}
        <button class:active={state.entities.source === option.value} class="button is-small" type="button" on:click={() => onSource(option.value)}>
          {option.label}
        </button>
      {/each}
    </div>
  </div>

  <div class="overview-body">
    <section class="overview-browser" aria-label="Project directory and asset overview">
      <div class="overview-section-title">
        <strong>Project directory</strong>
        <small>{state.project?.root_path ?? 'No project selected'}</small>
      </div>
      <div class="overview-row-list">
        {#each projectRows as row (row.id)}
          <button class:selected={isSelected(row.id)} class={`overview-row ${row.kind}`} type="button" on:click={() => selectProjectRow(row)}>
            <span>
              <strong>{row.title}</strong>
              <small>{row.subtitle ?? row.path ?? row.kind}</small>
            </span>
            <em>{row.kind}</em>
          </button>
        {:else}
          <p class="muted">No project items match the current query.</p>
        {/each}
      </div>
    </section>

    <section class="overview-entities" aria-label="Entity overview">
      <div class="overview-section-title">
        <strong>Entities</strong>
        <small>{state.entities.pageSize} page / {state.entities.overscanRows} overscan</small>
      </div>
      <div class="overview-entity-list" bind:this={scroller} on:scroll={handleScroll}>
        <div class="virtual-spacer" style={`height: ${totalRows * rowHeight}px;`}>
          {#each visibleIndexes as index (index)}
            {@const row = state.entities.rowsByIndex[index]}
            {#if row}
              <button
                class:selected={isSelected(row.id)}
                class="overview-entity-row"
                style={`transform: translateY(${index * rowHeight}px); height: ${rowHeight}px;`}
                type="button"
                on:click={() => selectEntity(row)}
              >
                <span class="entity-main">
                  <span class="entity-name">{row.name}</span>
                  <span class="entity-id mono">{row.id}</span>
                </span>
                <span class="entity-badges">
                  <DomainBadge domain={row.domain} />
                  <span class="mini-tag">{row.live_snapshot ? 'live' : 'static'}</span>
                  <span class="mini-tag">{row.component_count}</span>
                </span>
              </button>
            {:else}
              <div class="overview-entity-row placeholder" style={`transform: translateY(${index * rowHeight}px); height: ${rowHeight}px;`}>
                <span>loading {index}</span>
              </div>
            {/if}
          {/each}
        </div>
      </div>
    </section>
  </div>
</section>
