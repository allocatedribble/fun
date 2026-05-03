<script lang="ts">
  import type { EditorUiState } from '../types';

  export let state: EditorUiState;
  export let onEditor: () => void | Promise<void>;
  export let onLauncher: () => void | Promise<void>;
  export let onGame: (gameId: string) => void | Promise<void>;

  $: gameSlots = [
    state.games[0] ?? {
      id: 'game-1-placeholder',
      title: 'Game #1',
      status: 'offline',
      accent: 'gold'
    },
    state.games[1] ?? {
      id: 'game-2-placeholder',
      title: 'Game #2',
      status: 'offline',
      accent: 'blue'
    }
  ];

  async function selectGame(gameId: string): Promise<void> {
    if (!state.games.some((game) => game.id === gameId)) {
      await onLauncher();
      return;
    }
    await onGame(gameId);
  }
</script>

<aside class="mode-switch-pane" aria-label="Authorized mode switcher">
  <button
    class:active={state.launcherMode === 'editor'}
    class="mode-swatch editor"
    type="button"
    aria-label="Editor"
    title="Editor"
    on:click={onEditor}
  ></button>
  {#each gameSlots as game, index (game.id)}
    <button
      class:active={state.launcherMode !== 'editor' && state.selectedGameId === game.id}
      class={`mode-swatch game-${index + 1}`}
      type="button"
      aria-label={game.title}
      title={`${game.title} · ${game.status}`}
      on:click={() => selectGame(game.id)}
    ></button>
  {/each}
</aside>
