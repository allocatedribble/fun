<script lang="ts">
  import type { EditorUiState } from '../types';

  export let state: EditorUiState;
  export let onClose: () => void;
  export let onRequestTicket: (
    email: string,
    password: string,
    mode?: 'login' | 'register',
    displayName?: string
  ) => void | Promise<void>;
  export let onRefreshTicket: () => void | Promise<void>;
  export let onLogout: () => void | Promise<void>;

  let email = 'operator@fun.local';
  let password = '';
  let displayName = 'Fun Operator';
  let mode: 'login' | 'register' = 'login';

  $: capabilities = state.account.ticket?.capabilities ?? [];
  $: expiry = state.account.ticket ? new Date(state.account.ticket.expires_unix_ms).toLocaleTimeString() : '';

  async function submit(): Promise<void> {
    const submittedPassword = password;
    password = '';
    await onRequestTicket(email, submittedPassword, mode, displayName);
  }
</script>

<section class="account-login-panel" data-no-drag aria-label="Account login">
  <header>
    <div>
      <span class="eyebrow">{state.account.backendUrl}</span>
      <h2>{state.account.profile?.display_name ?? 'Account login'}</h2>
    </div>
    <button class="button is-small" type="button" on:click={onClose}>Close</button>
  </header>

  {#if state.account.profile}
    <div class="account-summary">
      <img src={state.account.profile.avatar_url} alt="" />
      <div>
        <strong>{state.account.profile.display_name}</strong>
        <span>{state.account.profile.handle}</span>
        <small>{state.account.ticket?.audience ?? state.account.backendUrl}</small>
      </div>
    </div>
  {/if}

  <form class="account-login-form" on:submit|preventDefault={submit}>
    <div class="account-mode-row">
      <button class:active={mode === 'login'} class="button is-small" type="button" on:click={() => (mode = 'login')}>Login</button>
      <button class:active={mode === 'register'} class="button is-small" type="button" on:click={() => (mode = 'register')}>Register</button>
    </div>

    {#if mode === 'register'}
      <label>
        <span>Display name</span>
        <input class="input is-small" autocomplete="name" bind:value={displayName} />
      </label>
    {/if}

    <label>
      <span>Email</span>
      <input class="input is-small" type="email" autocomplete="username" bind:value={email} />
    </label>
    <label>
      <span>Password</span>
      <input
        class="input is-small"
        type="password"
        autocomplete={mode === 'register' ? 'new-password' : 'current-password'}
        bind:value={password}
      />
    </label>
    <button class="button is-primary is-small" type="submit" disabled={state.account.loading}>
      {state.account.loading ? 'Working' : mode === 'register' ? 'Register' : 'Login'}
    </button>
  </form>

  {#if state.account.ticket}
    <section class="account-ticket-summary">
      <div>
        <span>{state.account.ticket.audience}</span>
        <strong>{expiry}</strong>
      </div>
      <ul>
        {#each capabilities as capability}
          <li>{capability}</li>
        {/each}
      </ul>
    </section>
  {/if}

  <div class="account-session-actions">
    <button class="button is-small" type="button" disabled={!state.account.ticket || state.account.loading} on:click={onRefreshTicket}>Refresh</button>
    <button class="button is-small" type="button" disabled={!state.account.ticket || state.account.loading} on:click={onLogout}>Logout</button>
  </div>

  {#if state.account.error}
    <p class="account-error">{state.account.error}</p>
  {/if}
</section>
