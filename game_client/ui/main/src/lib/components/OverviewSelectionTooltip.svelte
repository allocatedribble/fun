<script lang="ts">
  import type { Diagnostic, EditorUiState, TransformPatchRequest } from '../types';
  import DomainBadge from './DomainBadge.svelte';

  export let state: EditorUiState;
  export let onPatchTransform: (request: TransformPatchRequest) => void | Promise<void>;

  interface TransformFields {
    translation: [number, number, number];
    rotation: [number, number, number, number];
    scale: [number, number, number];
  }

  let transformEntityId: string | null = null;
  let tx = 0;
  let ty = 0;
  let tz = 0;
  let rx = 0;
  let ry = 0;
  let rz = 0;
  let rw = 1;
  let sx = 1;
  let sy = 1;
  let sz = 1;
  let persistIteration = false;

  function linkedEntityDiagnostics(diagnostics: Diagnostic[], entityId: string | null): Diagnostic[] {
    if (!entityId) {
      return [];
    }
    return diagnostics.filter(
      (diagnostic) =>
        diagnostic.hosted_instance_id === entityId ||
        diagnostic.message.includes(entityId) ||
        diagnostic.target_path?.includes(entityId)
    );
  }

  $: details = state.entities.selectedDetails;
  $: selection = state.overview.selectedItem;
  $: linkedDiagnostics = linkedEntityDiagnostics(state.diagnostics, details?.row.id ?? null);
  $: transform = details?.components.find((component) => component.name.includes('Transform'));
  $: canPatchTransform =
    details?.row.source_kind === 'server_live' && Boolean(transform) && transform?.read_only === false;
  $: if (details?.row.id !== transformEntityId) {
    transformEntityId = details?.row.id ?? null;
    const fields = parseTransformPreview(transform?.value_preview ?? null);
    tx = fields.translation[0];
    ty = fields.translation[1];
    tz = fields.translation[2];
    rx = fields.rotation[0];
    ry = fields.rotation[1];
    rz = fields.rotation[2];
    rw = fields.rotation[3];
    sx = fields.scale[0];
    sy = fields.scale[1];
    sz = fields.scale[2];
    persistIteration = false;
  }
  $: groupedComponents = details
    ? ['Identity', 'Transform', 'Network', 'Render', 'World', 'Diagnostics', 'Other'].map((group) => ({
        group,
        components: details.components.filter((component) => (component.ui_group ?? 'Other') === group)
      }))
    : [];
  $: componentCount = details?.components.length ?? 0;
  $: metadataEntries = Object.entries(selection?.metadata ?? {});

  function parseTransformPreview(preview: string | null): TransformFields {
    const fallback: TransformFields = {
      translation: [0, 0, 0],
      rotation: [0, 0, 0, 1],
      scale: [1, 1, 1]
    };
    if (!preview) {
      return fallback;
    }

    return {
      translation: parseTuple3(preview, /translation=\(([^)]+)\)/, fallback.translation),
      rotation: parseTuple4(preview, /rotation=\(([^)]+)\)/, fallback.rotation),
      scale: parseTuple3(preview, /scale=\(([^)]+)\)/, fallback.scale)
    };
  }

  function parseTuple3(source: string, pattern: RegExp, fallback: [number, number, number]): [number, number, number] {
    const values = parseTuple(source, pattern, 3);
    return values ? [values[0], values[1], values[2]] : fallback;
  }

  function parseTuple4(
    source: string,
    pattern: RegExp,
    fallback: [number, number, number, number]
  ): [number, number, number, number] {
    const values = parseTuple(source, pattern, 4);
    return values ? [values[0], values[1], values[2], values[3]] : fallback;
  }

  function parseTuple(source: string, pattern: RegExp, expected: number): number[] | null {
    const match = pattern.exec(source);
    if (!match) {
      return null;
    }
    const values = match[1].split(',').map((part) => Number.parseFloat(part.trim()));
    if (values.length !== expected || values.some((value) => !Number.isFinite(value))) {
      return null;
    }
    return values;
  }

  function finiteNumber(value: number, fallback: number): number {
    return Number.isFinite(value) ? value : fallback;
  }

  async function submitTransformPatch(): Promise<void> {
    if (!details || !canPatchTransform) {
      return;
    }
    await onPatchTransform({
      entity_id: details.row.id,
      base_revision: details.row.revision,
      translation: {
        x: finiteNumber(tx, 0),
        y: finiteNumber(ty, 0),
        z: finiteNumber(tz, 0)
      },
      rotation: {
        x: finiteNumber(rx, 0),
        y: finiteNumber(ry, 0),
        z: finiteNumber(rz, 0),
        w: finiteNumber(rw, 1)
      },
      scale: {
        x: finiteNumber(sx, 1),
        y: finiteNumber(sy, 1),
        z: finiteNumber(sz, 1)
      },
      persist_iteration: persistIteration
    });
  }
</script>

<aside class="overview-selection-tooltip" role="region" aria-label="Overview selection details">
  <div class="overview-tooltip-heading">
    <span class="eyebrow">Selection</span>
    <h2>{selection?.title ?? details?.row.name ?? state.project?.display_name ?? 'No selection'}</h2>
  </div>

  {#if details && (!selection || selection.kind === 'entity')}
    <div class="details-identity">
      <DomainBadge domain={details.row.domain} sourceKind={details.row.source_kind} />
      <span class="mini-tag">{details.row.live_snapshot ? 'live' : 'static'}</span>
      <span class="mini-tag">rev {details.row.revision}</span>
      <span class="mono entity-id-full">{details.row.id}</span>
    </div>

    <section class="details-section">
      <h3>Transform</h3>
      <div class="property-list">
        <article>
          <strong>{transform?.name ?? 'Transform'}</strong>
          <span>{transform?.value_preview ?? 'not exposed for this row'}</span>
        </article>
      </div>
      {#if canPatchTransform}
        <form class="transform-editor" on:submit|preventDefault={submitTransformPatch}>
          <div class="axis-grid">
            <label><span>X</span><input class="input is-small" type="number" step="0.05" bind:value={tx} /></label>
            <label><span>Y</span><input class="input is-small" type="number" step="0.05" bind:value={ty} /></label>
            <label><span>Z</span><input class="input is-small" type="number" step="0.05" bind:value={tz} /></label>
          </div>
          <div class="axis-grid compact">
            <label><span>RX</span><input class="input is-small" type="number" step="0.001" bind:value={rx} /></label>
            <label><span>RY</span><input class="input is-small" type="number" step="0.001" bind:value={ry} /></label>
            <label><span>RZ</span><input class="input is-small" type="number" step="0.001" bind:value={rz} /></label>
            <label><span>RW</span><input class="input is-small" type="number" step="0.001" bind:value={rw} /></label>
          </div>
          <div class="axis-grid">
            <label><span>SX</span><input class="input is-small" type="number" step="0.05" bind:value={sx} /></label>
            <label><span>SY</span><input class="input is-small" type="number" step="0.05" bind:value={sy} /></label>
            <label><span>SZ</span><input class="input is-small" type="number" step="0.05" bind:value={sz} /></label>
          </div>
          <div class="transform-actions">
            <label class="checkbox persist-toggle">
              <input type="checkbox" bind:checked={persistIteration} />
              <span>persist as iteration</span>
            </label>
            <button class="button is-primary is-small" type="submit">Apply</button>
          </div>
        </form>
      {/if}
    </section>

    <section class="details-section">
      <h3>Components</h3>
      <div class="property-list">
        {#if componentCount === 0}
          <p class="muted">No component details.</p>
        {:else}
          {#each groupedComponents as group}
            {#if group.components.length > 0}
              {#each group.components as component (`${component.component_kind ?? component.name}-${component.name}`)}
                <article>
                  <strong>{component.schema_label ?? component.name}</strong>
                  <span>{component.value_preview ?? 'available'}</span>
                  <small>{component.read_only ? 'read-only' : component.mutability ?? 'mutable'} / {component.replication_policy ?? 'local'}</small>
                </article>
              {/each}
            {/if}
          {/each}
        {/if}
      </div>
    </section>

    <section class="details-section">
      <h3>Resources</h3>
      <div class="property-list">
        {#each details.resources as resource (resource.name)}
          <article>
            <strong>{resource.name}</strong>
            <span>{resource.value_preview ?? resource.source ?? 'available'}</span>
          </article>
        {:else}
          <p class="muted">No resource metadata.</p>
        {/each}
      </div>
    </section>

    <section class="details-section">
      <h3>Network / authority</h3>
      <div class="property-list">
        <article><strong>Domain</strong><span>{details.row.domain}</span></article>
        <article><strong>Source</strong><span>{details.row.source_kind}</span></article>
        <article><strong>Revision</strong><span>{details.row.revision}</span></article>
      </div>
    </section>

    <section class="details-section">
      <h3>BSN source span</h3>
      {#if details.row.source_span}
        <p class="mono path-text">
          {details.row.source_span.path}:{details.row.source_span.start.line}:{details.row.source_span.start.column}
        </p>
      {:else}
        <p class="muted">No BSN span for this row.</p>
      {/if}
    </section>

    <section class="details-section">
      <h3>Linked diagnostics</h3>
      <div class="property-list">
        {#each linkedDiagnostics as diagnostic (`${diagnostic.code}-${diagnostic.message}`)}
          <article class={`diagnostic-row ${diagnostic.level}`}>
            <strong>{diagnostic.code}</strong>
            <span>{diagnostic.message}</span>
          </article>
        {:else}
          <p class="muted">No diagnostics linked.</p>
        {/each}
      </div>
    </section>

    {#if details.source_preview}
      <section class="details-section">
        <h3>Read-only source preview</h3>
        <pre>{details.source_preview.text}</pre>
      </section>
    {/if}
  {:else if selection}
    <div class="details-identity">
      <span class="mini-tag">{selection.kind}</span>
      {#if selection.scope}
        <span class="mini-tag">{selection.scope.type}</span>
      {/if}
      <span class="mono entity-id-full">{selection.id}</span>
    </div>

    <section class="details-section">
      <h3>Selection</h3>
      <div class="property-list">
        <article><strong>Title</strong><span>{selection.title}</span></article>
        <article><strong>Type</strong><span>{selection.kind}</span></article>
        {#if selection.subtitle}
          <article><strong>Summary</strong><span>{selection.subtitle}</span></article>
        {/if}
        {#if selection.path}
          <article><strong>Path</strong><span class="mono">{selection.path}</span></article>
        {/if}
      </div>
    </section>

    <section class="details-section">
      <h3>Metadata</h3>
      <div class="property-list">
        {#each metadataEntries as [key, value]}
          <article>
            <strong>{key}</strong>
            <span>{value === null ? 'none' : String(value)}</span>
          </article>
        {:else}
          <p class="muted">No metadata for this item.</p>
        {/each}
      </div>
    </section>
  {/if}
</aside>
