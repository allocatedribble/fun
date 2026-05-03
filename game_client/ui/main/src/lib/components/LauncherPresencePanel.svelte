<script lang="ts">
  import type { EditorUiState } from '../types';

  export let state: EditorUiState;

  $: active = state.activeRuntime;
  $: auth = state.authorization;
  $: events = state.events.slice(-5).reverse();
</script>

<aside class="launcher-presence-panel" aria-label="Presence and session">
  <section>
    <div class="launcher-section-head">
      <span class="eyebrow">Presence</span>
      <strong>{auth.can_edit_project ? 'authorized' : 'limited'}</strong>
    </div>
    <div class="presence-stack">
      <div>
        <span class="label">Session</span>
        <strong>{state.status?.auth_session.session_id ?? 'pending'}</strong>
      </div>
      <div>
        <span class="label">Capabilities</span>
        <strong>{auth.capabilities.length}</strong>
      </div>
      <div>
        <span class="label">Reason</span>
        <strong>{auth.reason ?? 'ready'}</strong>
      </div>
    </div>
  </section>

  <section>
    <div class="launcher-section-head">
      <span class="eyebrow">Runtime</span>
      <strong>{active?.status ?? 'idle'}</strong>
    </div>
    {#if active}
      <div class="presence-stack">
        <div>
          <span class="label">Instance</span>
          <strong>{active.instance_id}</strong>
        </div>
        <div>
          <span class="label">PID</span>
          <strong>{active.pid ?? 'n/a'}</strong>
        </div>
        <div>
          <span class="label">Inspector</span>
          <strong>{active.inspector_state}</strong>
        </div>
      </div>
    {:else}
      <p class="muted">No host runtime action is active.</p>
    {/if}
  </section>

  <section>
    <div class="launcher-section-head">
      <span class="eyebrow">Events</span>
      <strong>{events.length}</strong>
    </div>
    <div class="launcher-event-list">
      {#each events as event (`${event.timestamp}-${event.command_id}`)}
        <article class={`launcher-event ${event.level}`}>
          <strong>{event.command_id}</strong>
          <span>{event.message}</span>
        </article>
      {:else}
        <p class="muted">No launcher events yet.</p>
      {/each}
    </div>
  </section>
</aside>
