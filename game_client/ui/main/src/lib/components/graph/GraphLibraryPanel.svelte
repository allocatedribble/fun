<script lang="ts">
  import { nodeCategories, searchNodeDefinitions } from '../../graph/nodeRegistry';
  import type { GraphData, GraphPoint, NodeDefinition } from '../../graph/types';

  export let graph: GraphData;
  export let collapsed = false;
  export let onToggle: () => void;
  export let onCreateNode: (type: string, position?: GraphPoint) => void;

  let query = '';

  $: results = searchNodeDefinitions(query);
  $: byCategory = nodeCategories.map((category) => ({
    category,
    nodes: results.filter((definition) => definition.category === category)
  }));
  $: variables = graph.variables;

  function create(definition: NodeDefinition): void {
    onCreateNode(definition.type);
  }

  function handleDragStart(event: DragEvent, definition: NodeDefinition): void {
    event.dataTransfer?.setData('application/x-fun-graph-node', definition.type);
    event.dataTransfer?.setData('text/plain', definition.type);
    if (event.dataTransfer) {
      event.dataTransfer.effectAllowed = 'copy';
    }
  }
</script>

<aside class:collapsed class="graph-library-panel panel">
  <div class="graph-panel-head">
    <div>
      <span class="eyebrow">Node Library</span>
      <h2>{collapsed ? 'Library' : `${results.length} node types`}</h2>
    </div>
    <button class="button is-small" type="button" on:click={onToggle} title={collapsed ? 'Expand node library' : 'Collapse node library'}>
      {collapsed ? '>' : '<'}
    </button>
  </div>

  {#if !collapsed}
    <label class="field">
      <span class="label">Search</span>
      <span class="control">
        <input
          class="input is-small"
          type="search"
          placeholder="node, category, pin type"
          bind:value={query}
        />
      </span>
    </label>

    <div class="graph-library-scroll">
      {#each byCategory as group}
        {#if group.nodes.length > 0}
          <section class="graph-library-section">
            <h3>{group.category}</h3>
            <div class="graph-node-type-list">
              {#each group.nodes as definition (definition.type)}
                <button
                  type="button"
                  draggable="true"
                  on:dragstart={(event) => handleDragStart(event, definition)}
                  on:click={() => create(definition)}
                >
                  <strong>{definition.title}</strong>
                  <small>{definition.description}</small>
                </button>
              {/each}
            </div>
          </section>
        {/if}
      {/each}

      <section class="graph-library-section">
        <h3>Variables</h3>
        <div class="graph-variable-list">
          {#each variables as variable (variable.id)}
            <article>
              <strong>{variable.name}</strong>
              <small>{variable.type}</small>
            </article>
          {:else}
            <p class="muted">No graph variables yet.</p>
          {/each}
        </div>
      </section>

      <section class="graph-library-section">
        <h3>Assets</h3>
        <p class="muted">No asset nodes loaded.</p>
      </section>
    </div>
  {/if}
</aside>
