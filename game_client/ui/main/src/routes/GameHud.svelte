<script lang="ts">
  import { editorStore } from '../lib/stores/editor';
  import type { EditorUiState } from '../lib/types';

  export let state: EditorUiState;

  $: runtimeLabel = state.activeRuntime?.status ?? state.runtimeStatus?.client_process.state ?? 'local';
  $: objective = state.project?.display_name ?? state.selectedGameId ?? 'Fun';
</script>

<section class="game-hud-shell" aria-label="Game HUD">
  <div class="hud-cluster top-left" data-hit-region="minimap">
    <strong>{objective}</strong>
    <span>{runtimeLabel}</span>
  </div>

  <button
    class="hud-cluster bottom-left"
    type="button"
    data-hit-region="chat"
    on:click={editorStore.openGameChat}
  >
    <span>Squad</span>
    <strong>{state.runtimeStatus?.game_network.state ?? 'offline'}</strong>
  </button>

  <div class="hud-cluster bottom-right" data-hit-region="scoreboard">
    <span>Render</span>
    <strong>{state.runtimeStatus?.client_process.state ?? runtimeLabel}</strong>
  </div>

  <nav class="hud-actions" aria-label="HUD actions">
    <button type="button" data-hit-region="pause_menu" on:click={editorStore.openGameMenu}>Menu</button>
    <button type="button" data-hit-region="devtools" on:click={editorStore.toggleEditorOverlayShell}>Editor</button>
  </nav>
</section>

<style>
  .game-hud-shell {
    min-height: 100vh;
    position: relative;
    color: #f7efe0;
    background:
      linear-gradient(180deg, rgba(16, 19, 22, 0.18), rgba(16, 19, 22, 0.4)),
      radial-gradient(circle at 50% 35%, rgba(215, 168, 91, 0.18), transparent 34%),
      #101316;
    overflow: hidden;
  }

  .hud-cluster,
  .hud-actions {
    position: absolute;
    display: flex;
    align-items: center;
    gap: 0.55rem;
    min-height: 2.5rem;
    border: 1px solid rgba(247, 239, 224, 0.15);
    background: rgba(16, 19, 22, 0.64);
    backdrop-filter: blur(10px);
    padding: 0.55rem 0.75rem;
    border-radius: 0.375rem;
  }

  button.hud-cluster {
    color: inherit;
    font: inherit;
  }

  .hud-cluster span {
    color: rgba(247, 239, 224, 0.68);
    font-size: 0.78rem;
    text-transform: uppercase;
  }

  .top-left {
    top: 1rem;
    left: 1rem;
  }

  .bottom-left {
    bottom: 1rem;
    left: 1rem;
  }

  .bottom-right {
    right: 1rem;
    bottom: 1rem;
  }

  .hud-actions {
    top: 1rem;
    right: 1rem;
  }

  .hud-actions button {
    border: 1px solid rgba(247, 239, 224, 0.18);
    background: rgba(247, 239, 224, 0.08);
    color: #f7efe0;
    border-radius: 0.25rem;
    padding: 0.35rem 0.6rem;
    font: inherit;
  }
</style>
