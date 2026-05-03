<script lang="ts">
  import type { CommandDescriptor } from '../types';

  export let commands: CommandDescriptor[];
  export let search: string;
  export let onSearch: (value: string) => void;

  $: query = search.trim().toLowerCase();
  $: filtered = commands.filter((command) =>
    [
      command.id,
      command.title,
      command.summary,
      command.category,
      command.input_description,
      command.output_description
    ]
      .join(' ')
      .toLowerCase()
      .includes(query)
  );
</script>

<section class="drawer-panel">
  <div class="drawer-tools">
    <input
      class="input is-small"
      type="search"
      placeholder="Search commands"
      value={search}
      on:input={(event) => onSearch(event.currentTarget.value)}
    />
  </div>

  <div class="command-grid">
    {#each filtered as command (command.id)}
      <article>
        <div>
          <strong>{command.id}</strong>
          <span>{command.category}</span>
        </div>
        <p>{command.summary}</p>
        <small>{command.input_description} -> {command.output_description}</small>
      </article>
    {:else}
      <p class="muted">No commands match.</p>
    {/each}
  </div>
</section>
