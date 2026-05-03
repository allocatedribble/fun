<script lang="ts">
  import { editorStore } from '../stores/editor';
  import type { EditorUiState, JoinableGame } from '../types';
  import WindowTitleBar from './WindowTitleBar.svelte';

  export let state: EditorUiState;

  $: games = launcherGames(state.games);

  function launcherGames(source: JoinableGame[]): JoinableGame[] {
    const fallback = [
      {
        id: 'game-1-placeholder',
        title: 'Game #1',
        summary: 'New Fun experience slot.',
        region: 'local',
        status: 'unavailable',
        players_online: 0,
        max_players: 16,
        latency_ms: null,
        project_id: state.project?.id ?? state.projects[0]?.id ?? 'unassigned',
        build_profile: state.buildProfile,
        accent: 'gold'
      },
      {
        id: 'game-2-placeholder',
        title: 'Game #2',
        summary: 'New Fun experience slot.',
        region: 'local',
        status: 'unavailable',
        players_online: 0,
        max_players: 16,
        latency_ms: null,
        project_id: state.project?.id ?? state.projects[0]?.id ?? 'unassigned',
        build_profile: state.buildProfile,
        accent: 'blue'
      }
    ] satisfies JoinableGame[];
    return source.length >= 2 ? source.slice(0, 2) : [...source, ...fallback].slice(0, 2);
  }
</script>

<main class="launcher-shell compact-launcher">
  <WindowTitleBar mode="launcher" title="Fun" subtitle="" />

  {#if state.error}
    <section class="notification is-danger is-light shell-error">{state.error}</section>
  {/if}

  <section class="launcher-pane-grid" aria-label="Launcher modes">
    {#each games as game, index (game.id)}
      <button
        class:active={state.selectedGameId === game.id}
        class={`launcher-game-pane game-${index + 1}`}
        type="button"
        on:click={() => editorStore.selectGame(game.id)}
      >
        <span class="launcher-game-color"></span>
        <strong>{game.title || `Game #${index + 1}`}</strong>
        <small>{game.status} · {game.region}</small>
      </button>
    {/each}

    <button class="launcher-editor-pane" type="button" on:click={editorStore.activateEditorShell}>
      <span class="launcher-game-color editor"></span>
      <strong>Editor</strong>
      <small>{state.project?.display_name ?? state.projects[0]?.display_name ?? 'No project'}</small>
    </button>
  </section>
</main>
