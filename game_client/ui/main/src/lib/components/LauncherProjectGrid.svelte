<script lang="ts">
  import type { AuthorizedProject } from '../types';

  export let projects: AuthorizedProject[];
  export let selectedProjectId: string | null;
  export let onSelect: (projectId: string) => void;
</script>

<section class="launcher-grid-panel" aria-label="Projects">
  <div class="launcher-section-head">
    <span class="eyebrow">Projects</span>
    <strong>{projects.length}</strong>
  </div>

  <div class="launcher-card-grid project">
    {#each projects as project (project.id)}
      <button
        class:active={selectedProjectId === project.id}
        class="launcher-project-card"
        type="button"
        on:click={() => onSelect(project.id)}
      >
        <span class="launcher-card-title">{project.display_name}</span>
        <span class="launcher-card-summary mono">{project.root_path}</span>
        <span class="launcher-card-meta">
          <span>{project.crate_count} crates</span>
          <span>{project.bsn_scene_count} scenes</span>
          <span>{project.last_opened_label}</span>
        </span>
        <span class:locked={!project.edit_authorized} class="launcher-auth-chip">
          {project.edit_authorized ? 'edit authorized' : 'read only'}
        </span>
      </button>
    {:else}
      <p class="muted launcher-empty">No authorized projects registered.</p>
    {/each}
  </div>
</section>
