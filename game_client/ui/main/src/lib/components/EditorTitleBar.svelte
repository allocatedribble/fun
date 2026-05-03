<script lang="ts">
  import screwdriverIcon from '../../assets/icons/screwdriver.svg';
  import { toggleWindowMaximize } from '../windowChrome';
  import { isWindowDragExcluded, startWindowDrag } from '../windowDrag';
  import type { EditorUiState } from '../types';
  import AccountLoginPanel from './AccountLoginPanel.svelte';
  import Commandbar from './Commandbar.svelte';
  import WindowControls from './WindowControls.svelte';

  export let state: EditorUiState;
  export let onShowLauncher: () => void | Promise<void>;
  export let onDeactivateEditor: () => void | Promise<void>;
  export let onCommandInput: (value: string) => void;
  export let onCommandFocus: () => void;
  export let onCommandBlur: () => void;
  export let onCommandClear: () => void;
  export let onCommandMove: (delta: number) => void;
  export let onCommandExecute: (resultId?: string) => void | Promise<void>;
  export let onToggleAccount: () => void;
  export let onCloseAccount: () => void;
  export let onRequestAccountTicket: (
    email: string,
    password: string,
    mode?: 'login' | 'register',
    displayName?: string
  ) => void | Promise<void>;
  export let onRefreshAccountTicket: () => void | Promise<void>;
  export let onLogoutAccount: () => void | Promise<void>;

  $: canReturnToGame = state.activeEditorContextId === 'live_client' && state.liveClientStatus?.status === 'running';
  $: avatarUrl = state.account.profile?.avatar_url ?? '';
  $: avatarLabel = state.account.profile?.display_name?.slice(0, 1).toUpperCase() ?? '?';

  function onTitlebarDoubleClick(event: MouseEvent): void {
    if (event.button !== 0 || isWindowDragExcluded(event.target)) {
      return;
    }
    void toggleWindowMaximize();
  }

  function openAppMenu(): void {
    if (state.launcherMode === 'editor') {
      void onShowLauncher();
      return;
    }
    void onDeactivateEditor();
  }
</script>

<header class="compact-titlebar window-titlebar editor" aria-label="Editor window title bar">
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="titlebar-drag-zone compact-titlebar-left"
    data-fun-drag-region
    on:pointerdown={startWindowDrag}
    on:dblclick={onTitlebarDoubleClick}
  >
    <button
      class="screwdriver-menu-button"
      type="button"
      aria-label="Fun Editor Menu"
      title="Fun Editor Menu"
      data-no-drag
      on:click={openAppMenu}
    >
      <img src={screwdriverIcon} alt="" aria-hidden="true" />
    </button>
  </div>

  <div class="compact-titlebar-center" data-no-drag>
    <Commandbar
      {state}
      onInput={onCommandInput}
      onFocus={onCommandFocus}
      onBlur={onCommandBlur}
      onClear={onCommandClear}
      onMove={onCommandMove}
      onExecute={onCommandExecute}
    />
    <button
      class:authenticated={Boolean(state.account.ticket)}
      class="profile-avatar-button"
      type="button"
      data-no-drag
      aria-label="Account profile"
      title={state.account.profile?.display_name ?? 'Account login'}
      on:click={onToggleAccount}
    >
      {#if avatarUrl}
        <img src={avatarUrl} alt="" />
      {:else}
        <span>{avatarLabel}</span>
      {/if}
    </button>
    {#if state.account.loginOpen}
      <AccountLoginPanel
        {state}
        onClose={onCloseAccount}
        onRequestTicket={onRequestAccountTicket}
        onRefreshTicket={onRefreshAccountTicket}
        onLogout={onLogoutAccount}
      />
    {/if}
  </div>

  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="titlebar-drag-zone compact-titlebar-right"
    data-fun-drag-region
    on:pointerdown={startWindowDrag}
    on:dblclick={onTitlebarDoubleClick}
  ></div>

  <WindowControls mode="editor" {canReturnToGame} />
</header>
