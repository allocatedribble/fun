<script lang="ts">
  import type { ProjectSummary } from '../types';

  export let project: ProjectSummary | null;
</script>

<div class="project-breadcrumb">
  <span class="eyebrow">Project</span>
  <strong>{project?.display_name ?? 'No project selected'}</strong>
  <small>{project?.root_path ?? 'Open a project to inspect the current client preview'}</small>
</div>
