<script lang="ts">
  import { editorStore } from '../lib/stores/editor';
  import type { EditorUiState } from '../lib/types';

  export let state: EditorUiState;
</script>

<section class="game-menu-shell" aria-label="Pause menu">
  <div class="menu-panel" data-hit-region="pause_menu">
    <span>Paused</span>
    <strong>{state.selectedGameId ?? state.project?.display_name ?? 'Fun'}</strong>
    <div class="menu-actions">
      <button type="button" on:click={editorStore.hideLauncherShell}>Resume</button>
      <button type="button" on:click={editorStore.showLauncherShell}>Launcher</button>
      <button type="button" on:click={editorStore.activateEditorShell}>Editor</button>
    </div>
  </div>
</section>

<style>
  .game-menu-shell {
    min-height: 100vh;
    display: grid;
    place-items: center;
    background: rgba(16, 19, 22, 0.84);
    color: #f7efe0;
  }

  .menu-panel {
    width: min(24rem, calc(100vw - 2rem));
    border: 1px solid rgba(247, 239, 224, 0.16);
    background: rgba(24, 27, 30, 0.92);
    border-radius: 0.5rem;
    padding: 1rem;
    display: grid;
    gap: 0.75rem;
  }

  .menu-panel span {
    color: rgba(247, 239, 224, 0.64);
    text-transform: uppercase;
    font-size: 0.78rem;
  }

  .menu-actions {
    display: grid;
    gap: 0.5rem;
  }

  .menu-actions button {
    border: 1px solid rgba(247, 239, 224, 0.16);
    background: rgba(247, 239, 224, 0.08);
    color: #f7efe0;
    border-radius: 0.25rem;
    padding: 0.55rem 0.7rem;
    font: inherit;
    text-align: left;
  }
</style>
