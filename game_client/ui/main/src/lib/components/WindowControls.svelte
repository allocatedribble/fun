<script lang="ts">
  import { closeOrHideWindow, minimizeWindow, toggleWindowMaximize, type ChromeMode } from '../windowChrome';

  export let mode: ChromeMode = 'editor';
  export let canReturnToGame = false;
</script>

<div class="window-controls" aria-label="Window controls" data-no-drag>
  <button
    class="window-control minimize"
    type="button"
    aria-label="Minimize"
    title={mode === 'editor' && canReturnToGame ? 'Return input to gameplay, then minimize' : 'Minimize'}
    on:click={() => minimizeWindow(mode, canReturnToGame)}
  ></button>
  <button
    class="window-control maximize"
    type="button"
    aria-label="Maximize or restore"
    title="Maximize or restore"
    on:click={toggleWindowMaximize}
  ></button>
  <button
    class="window-control close"
    type="button"
    aria-label="Close"
    title={mode === 'editor' && canReturnToGame ? 'Return input to gameplay, then close' : 'Close window'}
    on:click={() => closeOrHideWindow(mode, canReturnToGame)}
  ></button>
</div>
