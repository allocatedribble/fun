<script lang="ts">
  import type { BuildProfile, EditorUiState } from '../types';
  import StatusPill from './StatusPill.svelte';
  import WindowControls from './WindowControls.svelte';

  export let state: EditorUiState;
  export let onProjectPath: (value: string) => void;
  export let onOpenPath: () => void | Promise<void>;
  export let onOpenDefault: () => void | Promise<void>;
  export let onPickProject: () => void | Promise<void>;
  export let onCommandSearch: (value: string) => void;
  export let onActiveScene: (value: string) => void;
  export let onBuildProfile: (value: BuildProfile) => void;
  export let onLaunchClient: () => void | Promise<void>;
  export let onLaunchServer: () => void | Promise<void>;
  export let onStopClient: () => void | Promise<void>;

  const profiles: { value: BuildProfile; label: string }[] = [
    { value: 'debug', label: 'debug' },
    { value: 'debug_diagnostics', label: 'debug+diagnostics' },
    { value: 'release', label: 'release' },
    { value: 'force_disable_dlss', label: 'debug no DLSS' }
  ];

  $: runtime = state.runtimeStatus ?? state.status?.runtime_status ?? null;
  $: scenes = state.project?.bsn_index.records ?? [];
  $: authState = state.status?.auth_session.expired ? 'expired' : 'ready';
</script>

<nav class="navbar topbar" aria-label="editor controls">
  <div class="topbar-titlebar">
    <div
      class="navbar-brand topbar-brand"
      data-fun-drag-region
    >
      <div class="navbar-item wordmark" data-fun-drag-region>
        <span data-fun-drag-region>Fun Editor</span>
      </div>
    </div>

    <div class="topbar-drag-fill" data-fun-drag-region></div>

    <WindowControls />
  </div>

  <div class="navbar-menu topbar-menu">
    <div class="navbar-start topbar-left">
      <label class="field path-field">
        <span class="label">Project path</span>
        <span class="control">
          <input
            class="input is-small"
            value={state.projectPath}
            spellcheck="false"
            on:input={(event) => onProjectPath(event.currentTarget.value)}
          />
        </span>
      </label>

      <label class="field scene-field">
        <span class="label">Active scene</span>
        <span class="select is-small">
          <select value={state.activeSceneId ?? ''} on:change={(event) => onActiveScene(event.currentTarget.value)}>
            <option value="">none</option>
            {#each scenes as scene (scene.id)}
              <option value={scene.id}>{scene.scene_function_name ?? scene.invocation_kind}</option>
            {/each}
          </select>
        </span>
      </label>
    </div>

    <div class="navbar-item command-palette">
      <label class="field">
        <span class="label">Command palette</span>
        <span class="control">
          <input
            class="input is-small"
            list="editor-command-ids"
            placeholder="Search command IDs"
            value={state.commandSearch}
            on:input={(event) => onCommandSearch(event.currentTarget.value)}
          />
        </span>
      </label>
      <datalist id="editor-command-ids">
        {#each state.commands as command (command.id)}
          <option value={command.id}>{command.title}</option>
        {/each}
      </datalist>
    </div>

    <div class="navbar-end topbar-right">
      <div class="navbar-item status-cluster">
        <StatusPill label="client" value={runtime?.client_process.state ?? 'unknown'} tone={runtime?.client_process.state === 'running' ? 'good' : 'neutral'} />
        <StatusPill label="server" value={runtime?.server_process.state ?? 'unknown'} tone={runtime?.server_process.state === 'running' ? 'good' : 'neutral'} />
        <StatusPill label="auth" value={authState} tone={authState === 'ready' ? 'good' : 'bad'} />
      </div>

      <div class="navbar-item launch-controls">
        <button class="button is-small" type="button" on:click={onPickProject} disabled={state.opening}>Browse</button>
        <button class="button is-small" type="button" on:click={onOpenPath} disabled={state.opening}>
          {state.opening ? 'Opening' : 'Open'}
        </button>
        <button class="button is-small is-link" type="button" on:click={onOpenDefault} disabled={state.opening}>Fun</button>
        <button class="button is-small is-primary" type="button" on:click={onLaunchClient} disabled={!state.project || state.launchingClient}>
          Client
        </button>
        <button class="button is-small is-primary" type="button" on:click={onLaunchServer} disabled={!state.project || state.launchingServer}>
          Server
        </button>
        <button class="button is-small" type="button" on:click={onStopClient} disabled={!state.viewport || state.stoppingClient}>
          Stop
        </button>
      </div>

      <label class="navbar-item field profile-field">
        <span class="label">Build</span>
        <span class="select is-small">
          <select value={state.buildProfile} on:change={(event) => onBuildProfile(event.currentTarget.value as BuildProfile)}>
            {#each profiles as profile}
              <option value={profile.value}>{profile.label}</option>
            {/each}
          </select>
        </span>
      </label>
    </div>
  </div>
</nav>
