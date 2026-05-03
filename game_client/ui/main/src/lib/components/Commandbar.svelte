<script lang="ts">
  import { tick } from 'svelte';
  import { groupedCommandbarResults, resultRisk } from '../commandbar/resultRanker';
  import { scopeLabel } from '../commandbar/scopeRegistry';
  import type { CommandbarResult, EditorUiState } from '../types';

  export let state: EditorUiState;
  export let onInput: (value: string) => void;
  export let onFocus: () => void;
  export let onBlur: () => void;
  export let onClear: () => void;
  export let onMove: (delta: number) => void;
  export let onExecute: (resultId?: string) => void | Promise<void>;

  let inputElement: HTMLInputElement | null = null;

  $: projectName = state.project?.display_name ?? 'Untitled Experience';
  $: projectPath = state.project?.root_path ?? state.projectPath ?? 'unsaved';
  $: identityMode = !state.commandbar.focused && state.commandbar.input.trim().length === 0;
  $: visibleResults = state.commandbar.results.slice(0, 12);
  $: groupedResults = groupedCommandbarResults(visibleResults);
  $: showGroupHeaders = groupedResults.length > 1;
  $: selectedResultId = state.commandbar.selectedResultId ?? visibleResults[0]?.id;

  $: if (state.commandbar.focused && inputElement) {
    void tick().then(() => inputElement?.focus());
  }

  function compactPath(path: string): string {
    if (path.length <= 54) {
      return path;
    }
    const normalized = path.replace(/\\/g, '/');
    const parts = normalized.split('/');
    if (parts.length <= 3) {
      return `${normalized.slice(0, 24)}...${normalized.slice(-24)}`;
    }
    return `${parts[0]}/.../${parts.slice(-2).join('/')}`;
  }

  function resultBadge(result: CommandbarResult): string {
    if (result.scope.type === 'client') {
      return 'Client';
    }
    if (result.scope.type === 'server') {
      return 'Server';
    }
    if (result.type === 'tool') {
      return 'Tool';
    }
    if (result.type === 'llm_answer') {
      return 'LLM';
    }
    if (result.scope.type === 'project_client_server') {
      return 'Project';
    }
    return result.type;
  }

  function onKeydown(event: KeyboardEvent): void {
    if (event.key === 'Escape') {
      event.preventDefault();
      if (state.commandbar.input.trim()) {
        onClear();
      } else {
        inputElement?.blur();
        onBlur();
      }
      return;
    }
    if (event.key === 'ArrowDown') {
      event.preventDefault();
      onMove(1);
      return;
    }
    if (event.key === 'ArrowUp') {
      event.preventDefault();
      onMove(-1);
      return;
    }
    if (event.key === 'Enter') {
      event.preventDefault();
      void onExecute(selectedResultId);
    }
  }

  function seedConversation(value: string): void {
    onInput(value);
    void tick().then(() => inputElement?.focus());
  }
</script>

<div class="commandbar-shell" data-no-drag>
  {#if identityMode}
    <button
      class="commandbar-identity"
      type="button"
      title={projectPath}
      aria-label="Open commandbar"
      on:click={onFocus}
    >
      <span class="commandbar-scope">{scopeLabel(state.commandbar.activeScope)}</span>
      <strong>{projectName}</strong>
      <small>{compactPath(projectPath)}</small>
    </button>
  {:else}
    <div class="commandbar-input-frame" class:ask-mode={state.commandbar.mode === 'ask'} class:tool-mode={state.commandbar.mode === 'tool'}>
      <span class="commandbar-scope">{scopeLabel(state.commandbar.activeScope)}</span>
      <input
        bind:this={inputElement}
        class="commandbar-input"
        value={state.commandbar.input}
        spellcheck="false"
        placeholder={`${projectName} - Search, command, ask, tool`}
        aria-label="Editor commandbar"
        on:focus={onFocus}
        on:blur={onBlur}
        on:keydown={onKeydown}
        on:input={(event) => onInput(event.currentTarget.value)}
      />
      <span class="commandbar-mode">{state.commandbar.mode}</span>
    </div>
  {/if}

  {#if state.commandbar.focused}
    <section class:has-query={Boolean(state.commandbar.input.trim())} class="commandbar-conversation commandbar-results" aria-label="Commandbar conversation">
      {#if state.commandbar.inferredIntent && state.commandbar.input.trim()}
        <div class="commandbar-intent">{state.commandbar.inferredIntent}</div>
      {/if}

      {#if state.commandbar.input.trim() && visibleResults.length > 0}
        <div class="commandbar-result-list">
          {#each groupedResults as group (group.group)}
            {#if showGroupHeaders}
              <span class="commandbar-result-group">{group.group}</span>
            {/if}
            {#each group.results as result (result.id)}
              {@const risk = resultRisk(result)}
              <button
                class:is-selected={selectedResultId === result.id}
                class={`commandbar-result ${result.type}`}
                type="button"
                on:mousedown|preventDefault={() => onExecute(result.id)}
              >
                <span class="result-icon">{result.icon ?? result.type.slice(0, 1).toUpperCase()}</span>
                <span>
                  <strong>{result.title}</strong>
                  {#if result.subtitle}
                    <small>{result.subtitle}</small>
                  {/if}
                </span>
                <span class="result-meta">
                  <em>{resultBadge(result)}</em>
                  {#if risk}
                    <em class={`risk-chip ${risk}`}>{risk}</em>
                  {/if}
                </span>
              </button>
            {/each}
          {/each}
        </div>
      {:else if state.commandbar.input.trim()}
        <p class="commandbar-empty">No indexed matches yet.</p>
      {:else}
        <div class="commandbar-conversation-start">
          <div>
            <strong>Command the editor</strong>
            <span>Search project data, open tabs, inspect selections, run preview/client actions, or ask for a diagnosis.</span>
          </div>
          <div class="conversation-suggestions" aria-label="Commandbar starters">
            <button type="button" on:mousedown|preventDefault={() => seedConversation('open overview')}>open overview</button>
            <button type="button" on:mousedown|preventDefault={() => seedConversation('open viewport')}>open viewport</button>
            <button type="button" on:mousedown|preventDefault={() => seedConversation('open graph')}>open graph</button>
            <button type="button" on:mousedown|preventDefault={() => seedConversation('show log')}>show log</button>
            <button type="button" on:mousedown|preventDefault={() => seedConversation('? why is preview unhealthy')}>? diagnose preview</button>
          </div>
        </div>
      {/if}

      {#if state.commandbar.pendingToolCall}
        <article class="tool-call-preview">
          <strong>{state.commandbar.pendingToolCall.title}</strong>
          <span>{state.commandbar.pendingToolCall.summary}</span>
          <small>{state.commandbar.pendingToolCall.affectedEntities.join(', ')}</small>
        </article>
      {/if}
    </section>
  {/if}
</div>
