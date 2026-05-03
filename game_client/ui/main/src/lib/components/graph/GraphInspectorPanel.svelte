<script lang="ts">
  import { getNodeDefinition, pinTypeLabels } from '../../graph/nodeRegistry';
  import { findConnection, findNode, firstSelection, pinRefsForConnection, selectedItems } from '../../graph/graphModel';
  import type { GraphData, GraphNode, GraphPropertyValue, GraphValidationIssue } from '../../graph/types';

  export let graph: GraphData;
  export let validationIssues: GraphValidationIssue[];
  export let collapsed = false;
  export let onToggle: () => void;
  export let onRenameNode: (node: GraphNode, title: string) => void;
  export let onPropertyCommit: (node: GraphNode, property: string, value: GraphPropertyValue) => void;
  export let onDeleteSelection: () => void;
  export let onDuplicateSelection: () => void;
  export let onArrangeSelection: () => void;
  export let onGraphName: (name: string) => void;
  export let onGraphDescription: (description: string) => void;

  $: selection = selectedItems(graph);
  $: primarySelection = firstSelection(graph);
  $: selectedNode = primarySelection?.kind === 'node' ? findNode(graph, primarySelection.id) ?? null : null;
  $: selectedConnection =
    primarySelection?.kind === 'connection' ? findConnection(graph, primarySelection.id) ?? null : null;
  $: connectionRefs = selectedConnection ? pinRefsForConnection(graph, selectedConnection) : null;
  $: nodeIssues = selectedNode ? validationIssues.filter((issue) => issue.nodeId === selectedNode.id) : [];
  $: graphErrors = validationIssues.filter((issue) => issue.level === 'error').length;
  $: graphWarnings = validationIssues.filter((issue) => issue.level === 'warning').length;

  function propertyInputType(value: GraphPropertyValue): string {
    return typeof value === 'number' ? 'number' : typeof value === 'boolean' ? 'checkbox' : 'text';
  }

  function parsePropertyValue(value: GraphPropertyValue, event: Event): GraphPropertyValue {
    const target = event.currentTarget as HTMLInputElement;
    if (typeof value === 'number') {
      const parsed = Number.parseFloat(target.value);
      return Number.isFinite(parsed) ? parsed : value;
    }
    if (typeof value === 'boolean') {
      return target.checked;
    }
    return target.value;
  }
</script>

<aside class:collapsed class="graph-inspector-panel panel">
  <div class="graph-panel-head">
    <div>
      <span class="eyebrow">Inspector</span>
      <h2>{collapsed ? 'Details' : selectedNode?.title ?? (selectedConnection ? 'Connection' : 'Graph')}</h2>
    </div>
    <button class="button is-small" type="button" on:click={onToggle} title={collapsed ? 'Expand inspector' : 'Collapse inspector'}>
      {collapsed ? '<' : '>'}
    </button>
  </div>

  {#if !collapsed}
    <div class="graph-inspector-scroll">
      {#if selection.length > 1}
        <section class="graph-details-section">
          <h3>Selection</h3>
          <div class="property-list">
            <article><strong>Items</strong><span>{selection.length}</span></article>
          </div>
          <div class="graph-action-row">
            <button class="button is-small" type="button" on:click={onArrangeSelection}>Arrange</button>
            <button class="button is-small" type="button" on:click={onDuplicateSelection}>Duplicate</button>
            <button class="button is-small is-danger" type="button" on:click={onDeleteSelection}>Delete</button>
          </div>
        </section>
      {:else if selectedNode}
        {@const definition = getNodeDefinition(selectedNode.type)}
        <section class="graph-details-section">
          <h3>Node</h3>
          <label class="field">
            <span class="label">Title</span>
            <input
              class="input is-small"
              value={selectedNode.title}
              on:change={(event) => onRenameNode(selectedNode, event.currentTarget.value)}
            />
          </label>
          <div class="property-list">
            <article><strong>Type</strong><span>{selectedNode.type}</span></article>
            <article><strong>Category</strong><span>{definition?.category ?? 'Unknown'}</span></article>
            <article><strong>ID</strong><span class="mono">{selectedNode.id}</span></article>
          </div>
        </section>

        <section class="graph-details-section">
          <h3>Properties</h3>
          {#if Object.keys(selectedNode.properties).length > 0}
            <div class="graph-property-editor">
              {#each Object.entries(selectedNode.properties) as [property, value] (property)}
                <label class:checkbox-row={propertyInputType(value) === 'checkbox'} class="field">
                  <span class="label">{property}</span>
                  {#if propertyInputType(value) === 'checkbox'}
                    <input
                      type="checkbox"
                      checked={Boolean(value)}
                      on:change={(event) => onPropertyCommit(selectedNode, property, parsePropertyValue(value, event))}
                    />
                  {:else if typeof value === 'string' && value.startsWith('#')}
                    <input
                      class="input is-small"
                      type="color"
                      value={value}
                      on:change={(event) => onPropertyCommit(selectedNode, property, parsePropertyValue(value, event))}
                    />
                  {:else}
                    <input
                      class="input is-small"
                      type={propertyInputType(value)}
                      value={String(value ?? '')}
                      on:change={(event) => onPropertyCommit(selectedNode, property, parsePropertyValue(value, event))}
                    />
                  {/if}
                </label>
              {/each}
            </div>
          {:else}
            <p class="muted">No editable properties.</p>
          {/if}
        </section>

        <section class="graph-details-section">
          <h3>Pins</h3>
          <div class="property-list">
            {#each selectedNode.inputs as pin (pin.id)}
              <article><strong>{pin.name}</strong><span>input {pinTypeLabels[pin.type]}</span></article>
            {/each}
            {#each selectedNode.outputs as pin (pin.id)}
              <article><strong>{pin.name}</strong><span>output {pinTypeLabels[pin.type]}</span></article>
            {/each}
            {#if selectedNode.inputs.length + selectedNode.outputs.length === 0}
              <p class="muted">No pins.</p>
            {/if}
          </div>
        </section>

        <section class="graph-details-section">
          <h3>Validation</h3>
          <div class="property-list">
            {#each nodeIssues as issue (issue.id)}
              <article class={`graph-validation-row ${issue.level}`}>
                <strong>{issue.level}</strong>
                <span>{issue.message}</span>
              </article>
            {:else}
              <p class="muted">No node issues.</p>
            {/each}
          </div>
        </section>
      {:else if selectedConnection && connectionRefs}
        <section class="graph-details-section">
          <h3>Connection</h3>
          <div class="property-list">
            <article><strong>Source</strong><span>{connectionRefs.fromNode?.title ?? selectedConnection.fromNodeId} / {connectionRefs.fromPin?.name ?? selectedConnection.fromPinId}</span></article>
            <article><strong>Target</strong><span>{connectionRefs.toNode?.title ?? selectedConnection.toNodeId} / {connectionRefs.toPin?.name ?? selectedConnection.toPinId}</span></article>
            <article><strong>Type</strong><span>{connectionRefs.fromPin?.type ?? 'unknown'}</span></article>
          </div>
          <div class="graph-action-row">
            <button class="button is-small is-danger" type="button" on:click={onDeleteSelection}>Delete</button>
          </div>
        </section>
      {:else}
        <section class="graph-details-section">
          <h3>Graph</h3>
          <label class="field">
            <span class="label">Name</span>
            <input class="input is-small" value={graph.name} on:change={(event) => onGraphName(event.currentTarget.value)} />
          </label>
          <label class="field">
            <span class="label">Description</span>
            <input
              class="input is-small"
              value={graph.metadata.description ?? ''}
              on:change={(event) => onGraphDescription(event.currentTarget.value)}
            />
          </label>
          <div class="property-list">
            <article><strong>Schema</strong><span>v{graph.version}</span></article>
            <article><strong>Nodes</strong><span>{graph.nodes.length}</span></article>
            <article><strong>Connections</strong><span>{graph.connections.length}</span></article>
            <article><strong>Validation</strong><span>{graphErrors} errors / {graphWarnings} warnings</span></article>
          </div>
        </section>
      {/if}
    </div>
  {/if}
</aside>
