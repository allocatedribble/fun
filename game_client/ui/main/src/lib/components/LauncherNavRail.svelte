<script lang="ts">
  import type { LauncherMode, LauncherTab } from '../types';

  export let activeTab: LauncherTab;
  export let launcherMode: LauncherMode;
  export let onTab: (tab: LauncherTab) => void;

  const tabs: { value: LauncherTab; label: string; glyph: string }[] = [
    { value: 'games', label: 'Games', glyph: 'G' },
    { value: 'projects', label: 'Projects', glyph: 'P' },
    { value: 'settings', label: 'Settings', glyph: 'S' }
  ];
</script>

<aside class="launcher-rail" aria-label="Launcher navigation">
  <div class="launcher-rail-mark" title="Fun">F</div>
  <nav class="launcher-rail-buttons">
    {#each tabs as tab}
      <button
        class:active={activeTab === tab.value}
        type="button"
        aria-label={tab.label}
        title={tab.label}
        on:click={() => onTab(tab.value)}
      >
        {tab.glyph}
      </button>
    {/each}
  </nav>
  <div class="launcher-rail-mode" title={`Mode: ${launcherMode}`}>{launcherMode.slice(0, 1).toUpperCase()}</div>
</aside>
