<script lang="ts">
  import type { JoinableGame } from '../types';

  export let games: JoinableGame[];
  export let selectedGameId: string | null;
  export let onSelect: (gameId: string) => void;
</script>

<section class="launcher-grid-panel" aria-label="Games">
  <div class="launcher-section-head">
    <span class="eyebrow">Games</span>
    <strong>{games.length}</strong>
  </div>

  <div class="launcher-card-grid">
    {#each games as game (game.id)}
      <button
        class:active={selectedGameId === game.id}
        class="launcher-game-card"
        type="button"
        on:click={() => onSelect(game.id)}
      >
        <span class={`launcher-card-accent ${game.accent}`}></span>
        <span class="launcher-card-title">{game.title}</span>
        <span class="launcher-card-summary">{game.summary}</span>
        <span class="launcher-card-meta">
          <span>{game.region}</span>
          <span>{game.players_online}/{game.max_players}</span>
          <span>{game.latency_ms === null ? 'local' : `${game.latency_ms} ms`}</span>
        </span>
      </button>
    {:else}
      <p class="muted launcher-empty">No launcher games registered.</p>
    {/each}
  </div>
</section>
