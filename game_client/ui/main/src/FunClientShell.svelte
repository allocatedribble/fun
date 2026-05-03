<script lang="ts">
  import { onMount } from 'svelte';
  import ModeSwitchPane from './lib/components/ModeSwitchPane.svelte';
  import { reportHostHitRegions } from './lib/host/hitRegions';
  import { installMaterialRipples } from './lib/materialRipple';
  import { routeForState, type FunClientRoute } from './lib/routes';
  import { isHostRuntime } from './lib/commands';
  import SharedOverlayLayer from './lib/components/SharedOverlayLayer.svelte';
  import { editorStore } from './lib/stores/editor';
  import DiagnosticsShell from './routes/DiagnosticsShell.svelte';
  import EditorShell from './routes/EditorShell.svelte';
  import GameChat from './routes/GameChat.svelte';
  import GameHud from './routes/GameHud.svelte';
  import GameLoading from './routes/GameLoading.svelte';
  import GamePauseMenu from './routes/GamePauseMenu.svelte';
  import GameScoreboard from './routes/GameScoreboard.svelte';
  import LauncherShell from './routes/LauncherShell.svelte';

  let hitRegionFrame = 0;
  let route: FunClientRoute = 'loading';

  function scheduleHitRegionReport(): void {
    cancelAnimationFrame(hitRegionFrame);
    hitRegionFrame = requestAnimationFrame(() => {
      const captureMode = route === 'game.hud' ? 'hud_passive' : 'ui_modal';
      reportHostHitRegions(captureMode);
    });
  }

  function isTextEntryTarget(target: EventTarget | null): boolean {
    if (!(target instanceof HTMLElement)) {
      return false;
    }
    return (
      target.isContentEditable ||
      target instanceof HTMLInputElement ||
      target instanceof HTMLTextAreaElement ||
      target instanceof HTMLSelectElement
    );
  }

  function handleGlobalKeydown(event: KeyboardEvent): void {
    if ((event.ctrlKey || event.metaKey) && !event.altKey && event.key.toLowerCase() === 'k') {
      event.preventDefault();
      editorStore.focusCommandbar();
      return;
    }

    if (event.key === 'F1') {
      event.preventDefault();
      void editorStore.toggleEditorOverlayShell();
      return;
    }

    if (event.key !== 'Escape') {
      return;
    }

    if (isTextEntryTarget(event.target)) {
      return;
    }

    event.preventDefault();
    if ($editorStore.commandbar.focused) {
      editorStore.blurCommandbar();
      return;
    }
    if (route === 'game.hud') {
      void editorStore.openGameMenu();
      return;
    }
    void editorStore.hideLauncherShell();
  }

  onMount(() => {
    const uninstallRipples = installMaterialRipples();
    void editorStore.initialize();
    const heartbeat = isHostRuntime()
      ? null
      : setInterval(() => {
          void editorStore.refreshTrace();
        }, 1_000);
    window.addEventListener('resize', scheduleHitRegionReport);
    window.addEventListener('keydown', handleGlobalKeydown);
    scheduleHitRegionReport();

    return () => {
      uninstallRipples();
      if (heartbeat) {
        clearInterval(heartbeat);
      }
      cancelAnimationFrame(hitRegionFrame);
      window.removeEventListener('resize', scheduleHitRegionReport);
      window.removeEventListener('keydown', handleGlobalKeydown);
    };
  });

  $: route = routeForState($editorStore);
  $: showModeSwitch =
    $editorStore.authorization.can_open_project ||
    $editorStore.authorization.can_edit_project ||
    $editorStore.authorization.can_resume_editor ||
    $editorStore.authorization.can_join_game ||
    $editorStore.authorization.capabilities.length > 0;
  $: route, scheduleHitRegionReport();
</script>

<main class:authorized-mode-pane={showModeSwitch} class="fun-app-root" data-route={route}>
  {#if showModeSwitch && route !== 'game.hud'}
    <ModeSwitchPane
      state={$editorStore}
      onEditor={editorStore.activateEditorShell}
      onLauncher={editorStore.showLauncherShell}
      onGame={editorStore.selectGameAndShowLauncher}
    />
  {/if}

  <section class="fun-mode-host">
    {#if route === 'loading'}
      <GameLoading />
    {:else if route === 'game.hud'}
      <GameHud state={$editorStore} />
    {:else if route === 'game.pause'}
      <GameHud state={$editorStore} />
      <GamePauseMenu state={$editorStore} />
    {:else if route === 'game.chat'}
      <GameHud state={$editorStore} />
      <GameChat state={$editorStore} />
    {:else if route === 'game.scoreboard'}
      <GameHud state={$editorStore} />
      <GameScoreboard state={$editorStore} />
    {:else if route === 'editor.diagnostics'}
      <DiagnosticsShell state={$editorStore} />
    {:else if route.startsWith('editor.')}
      <EditorShell />
    {:else}
      <LauncherShell state={$editorStore} />
    {/if}
  </section>

  <SharedOverlayLayer state={$editorStore} {route} />
</main>
