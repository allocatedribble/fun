<script lang="ts">
  import type { AuthorizedProject, JoinableGame, LauncherActiveRuntime, LauncherTab } from '../types';

  export let activeTab: LauncherTab;
  export let selectedGame: JoinableGame | null;
  export let selectedProject: AuthorizedProject | null;
  export let activeRuntime: LauncherActiveRuntime | null;
  export let canJoin: boolean;
  export let canHost: boolean;
  export let canEdit: boolean;
  export let canResume: boolean;
  export let canStop: boolean;
  export let onJoin: () => void | Promise<void>;
  export let onHost: () => void | Promise<void>;
  export let onOpenProject: () => void | Promise<void>;
  export let onEditProject: () => void | Promise<void>;
  export let onResumeEditor: () => void | Promise<void>;
  export let onStopRuntime: () => void | Promise<void>;
  export let onSettings: () => void;

  $: title =
    activeTab === 'projects'
      ? selectedProject?.display_name ?? 'No project selected'
      : activeTab === 'settings'
        ? 'Launcher Settings'
        : selectedGame?.title ?? 'No game selected';
  $: summary =
    activeTab === 'projects'
      ? selectedProject?.root_path ?? 'Select an authorized project.'
      : activeTab === 'settings'
        ? 'Runtime ownership, build profiles, project authorization, and editor activation stay in Rust.'
        : selectedGame?.summary ?? 'Select a join target.';
  $: joinReason = !selectedGame ? 'No game selected.' : !canJoin ? 'Join authorization is unavailable.' : 'Join Game';
  $: hostReason = canHost ? 'Host Local' : 'Host authorization is unavailable.';
  $: openReason = selectedProject ? 'Open Project' : 'No project selected.';
  $: editReason = !selectedProject ? 'No project selected.' : !canEdit ? 'Edit authorization is unavailable.' : 'Edit Project';
  $: resumeReason = canResume ? 'Resume Editor' : 'No editor session can be resumed.';
  $: stopReason = activeRuntime ? (canStop ? 'Stop Runtime' : 'Stop authorization is unavailable.') : 'No active runtime.';
</script>

<section class="launcher-hero-panel" aria-label="Selected launcher item">
  <div class="launcher-hero-art" aria-hidden="true">
    <div class="hero-grid-line"></div>
    <div class="hero-block a"></div>
    <div class="hero-block b"></div>
    <div class="hero-block c"></div>
  </div>

  <div class="launcher-hero-content">
    <span class="eyebrow">{activeTab === 'projects' ? 'Project' : activeTab === 'settings' ? 'Settings' : 'Selected game'}</span>
    <h1>{title}</h1>
    <p>{summary}</p>

    <div class="launcher-hero-meta">
      {#if selectedGame && activeTab === 'games'}
        <span>{selectedGame.region}</span>
        <span>{selectedGame.players_online}/{selectedGame.max_players} online</span>
        <span>{selectedGame.build_profile}</span>
      {:else if selectedProject && activeTab === 'projects'}
        <span>{selectedProject.crate_count} crates</span>
        <span>{selectedProject.bsn_scene_count} scenes</span>
        <span>{selectedProject.authorization_summary}</span>
      {:else}
        <span>host controlled</span>
        <span>typed commands</span>
        <span>no webview authority</span>
      {/if}
    </div>

    <div class="launcher-actions">
      {#if activeTab === 'games'}
        <button class="button is-primary" type="button" on:click={onJoin} disabled={!selectedGame || !canJoin} title={joinReason}>Join Game</button>
        <button class="button" type="button" on:click={onHost} disabled={!canHost} title={hostReason}>Host Local</button>
      {:else if activeTab === 'projects'}
        <button class="button is-primary" type="button" on:click={onOpenProject} disabled={!selectedProject} title={openReason}>Open Project</button>
        <button class="button" type="button" on:click={onEditProject} disabled={!selectedProject || !canEdit} title={editReason}>Edit Project</button>
      {:else}
        <button class="button is-primary" type="button" on:click={onSettings}>Settings</button>
      {/if}
      <button class="button" type="button" on:click={onResumeEditor} disabled={!canResume} title={resumeReason}>Resume Editor</button>
      <button class="button" type="button" on:click={onStopRuntime} disabled={!activeRuntime || !canStop} title={stopReason}>Stop Runtime</button>
    </div>
  </div>
</section>
