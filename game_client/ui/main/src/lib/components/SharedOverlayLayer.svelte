<script lang="ts">
  import type { FunClientRoute } from '../routes';
  import { editorStore } from '../stores/editor';
  import type { EditorUiState } from '../types';
  import AccountLoginPanel from './AccountLoginPanel.svelte';
  import Commandbar from './Commandbar.svelte';
  import LogPanel from './LogPanel.svelte';
  import UiFrameRateBadge from './UiFrameRateBadge.svelte';

  export let state: EditorUiState;
  export let route: FunClientRoute;

  $: editorRoute = route.startsWith('editor.');
  $: showUiFrameRate = route.startsWith('launcher.') || editorRoute;
  $: showCommandbar = !editorRoute && (state.commandbar.focused || state.commandbar.input.trim().length > 0);
  $: showDiagnostics = state.diagnosticsView.visible && route !== 'editor.diagnostics';
  $: notificationCount = state.diagnosticsView.unreadCriticalCount + state.diagnosticsView.unreadWarningCount;
</script>

<div class="shared-overlay-layer" aria-label="Shared overlay">
  {#if showUiFrameRate}
    <div class="shared-ui-frame-rate">
      <UiFrameRateBadge />
    </div>
  {/if}

  {#if showCommandbar}
    <div class="shared-commandbar" data-hit-region="commandbar">
      <Commandbar
        {state}
        onInput={editorStore.setCommandbarInput}
        onFocus={editorStore.focusCommandbar}
        onBlur={editorStore.blurCommandbar}
        onClear={editorStore.clearCommandbar}
        onMove={editorStore.moveCommandbarSelection}
        onExecute={editorStore.executeCommandbarResult}
      />
    </div>
  {/if}

  {#if !editorRoute}
    <div class="shared-account" data-hit-region="account">
      <button
        class:authenticated={Boolean(state.account.ticket)}
        type="button"
        aria-label="Account profile"
        title={state.account.profile?.display_name ?? 'Account login'}
        on:click={editorStore.toggleAccountPanel}
      >
        {state.account.profile?.display_name?.slice(0, 1).toUpperCase() ?? '?'}
      </button>
      {#if state.account.loginOpen}
        <AccountLoginPanel
          {state}
          onClose={editorStore.closeAccountPanel}
          onRequestTicket={editorStore.requestAccountTicket}
          onRefreshTicket={editorStore.refreshAccountTicket}
          onLogout={editorStore.logoutAccount}
        />
      {/if}
    </div>
  {/if}

  {#if notificationCount > 0 && !showDiagnostics}
    <button
      class="shared-notifications"
      type="button"
      data-hit-region="notifications"
      aria-label="Open diagnostics"
      on:click={editorStore.toggleDiagnostics}
    >
      {notificationCount}
    </button>
  {/if}

  {#if showDiagnostics}
    <aside class="shared-log-drawer" data-hit-region="diagnostics">
      <LogPanel {state} />
    </aside>
  {/if}
</div>

<style>
  .shared-overlay-layer {
    position: fixed;
    inset: 0;
    z-index: 20;
    pointer-events: none;
  }

  .shared-commandbar,
  .shared-account,
  .shared-notifications,
  .shared-log-drawer {
    pointer-events: auto;
  }

  .shared-ui-frame-rate {
    position: absolute;
    top: 0.75rem;
    right: 3.35rem;
    pointer-events: none;
  }

  .shared-commandbar {
    position: absolute;
    top: 0.75rem;
    left: 50%;
    width: min(46rem, calc(100vw - 2rem));
    transform: translateX(-50%);
  }

  .shared-account {
    position: absolute;
    top: 0.75rem;
    right: 0.75rem;
  }

  .shared-account > button,
  .shared-notifications {
    width: 2.25rem;
    aspect-ratio: 1;
    border: 1px solid rgba(247, 239, 224, 0.18);
    border-radius: 0.375rem;
    background: rgba(16, 19, 22, 0.72);
    color: #f7efe0;
    font: inherit;
  }

  .shared-account > button.authenticated {
    border-color: rgba(124, 191, 141, 0.55);
  }

  .shared-notifications {
    position: absolute;
    right: 0.75rem;
    bottom: 0.75rem;
  }

  .shared-log-drawer {
    position: absolute;
    left: 1rem;
    right: 1rem;
    bottom: 1rem;
    max-height: min(32rem, calc(100vh - 7rem));
    overflow: auto;
  }
</style>
