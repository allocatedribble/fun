<script lang="ts">
  import type { EditorUiState } from '../lib/types';

  export let state: EditorUiState;
</script>

<section class="game-scoreboard-shell" aria-label="Scoreboard" data-hit-region="scoreboard">
  <header>
    <span>Scoreboard</span>
    <strong>{state.selectedGameId ?? state.project?.display_name ?? 'Fun'}</strong>
  </header>
  <div class="score-grid">
    <article><span>Client</span><strong>{state.runtimeStatus?.client_process.state ?? 'local'}</strong></article>
    <article><span>Server</span><strong>{state.runtimeStatus?.server_process.state ?? 'idle'}</strong></article>
    <article><span>Network</span><strong>{state.runtimeStatus?.game_network.state ?? 'offline'}</strong></article>
  </div>
</section>

<style>
  .game-scoreboard-shell {
    position: absolute;
    inset: 5rem 50%;
    z-index: 5;
    width: min(36rem, calc(100vw - 2rem));
    height: fit-content;
    transform: translateX(-50%);
    border: 1px solid rgba(247, 239, 224, 0.16);
    background: rgba(16, 19, 22, 0.82);
    border-radius: 0.5rem;
    color: #f7efe0;
    padding: 0.9rem;
  }

  .game-scoreboard-shell header,
  .score-grid {
    display: grid;
    gap: 0.65rem;
  }

  .score-grid {
    grid-template-columns: repeat(3, minmax(0, 1fr));
    margin-top: 0.8rem;
  }

  .score-grid article {
    border: 1px solid rgba(247, 239, 224, 0.12);
    border-radius: 0.375rem;
    padding: 0.65rem;
    background: rgba(247, 239, 224, 0.06);
  }

  span {
    color: rgba(247, 239, 224, 0.64);
    text-transform: uppercase;
    font-size: 0.76rem;
  }
</style>
