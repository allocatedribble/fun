<script lang="ts">
  import { onMount } from 'svelte';
  import { editorStore } from '../lib/stores/editor';
  import type { EditorUiState } from '../lib/types';

  export let state: EditorUiState;

  let inputElement: HTMLInputElement | null = null;

  onMount(() => {
    inputElement?.focus();
  });
</script>

<section class="game-chat-shell" aria-label="Chat" data-hit-region="chat">
  <div>
    <span>Squad</span>
    <strong>{state.runtimeStatus?.game_network.state ?? 'offline'}</strong>
  </div>
  <input
    bind:this={inputElement}
    aria-label="Chat message"
    placeholder="Message"
    on:keydown={(event) => {
      if (event.key === 'Escape') {
        event.currentTarget.blur();
        void editorStore.hideLauncherShell();
      }
    }}
  />
</section>

<style>
  .game-chat-shell {
    position: absolute;
    left: 1rem;
    bottom: 1rem;
    z-index: 5;
    width: min(28rem, calc(100vw - 2rem));
    display: grid;
    gap: 0.5rem;
    color: #f7efe0;
  }

  .game-chat-shell div,
  .game-chat-shell input {
    border: 1px solid rgba(247, 239, 224, 0.16);
    background: rgba(16, 19, 22, 0.78);
    border-radius: 0.375rem;
    padding: 0.6rem 0.75rem;
  }

  .game-chat-shell div {
    display: flex;
    justify-content: space-between;
    gap: 1rem;
  }

  .game-chat-shell span {
    color: rgba(247, 239, 224, 0.64);
    text-transform: uppercase;
    font-size: 0.76rem;
  }

  .game-chat-shell input {
    color: #f7efe0;
    font: inherit;
  }
</style>
