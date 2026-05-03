<script lang="ts">
  import { uiFrameRate } from '../stores/uiPerformance';

  $: fpsLabel = $uiFrameRate.fps > 0 ? Math.round($uiFrameRate.fps).toString() : '--';
  $: frameMsLabel = $uiFrameRate.frame_ms > 0 ? $uiFrameRate.frame_ms.toFixed(1) : '--';
</script>

<div class="ui-frame-rate-badge" aria-label="CEF Svelte UI frame rate">
  <span>UI</span>
  <strong>{fpsLabel} fps</strong>
  <small>{frameMsLabel} ms</small>
</div>

<style>
  .ui-frame-rate-badge {
    display: inline-grid;
    grid-template-columns: auto auto;
    gap: 0.125rem 0.45rem;
    align-items: baseline;
    min-width: 6.4rem;
    padding: 0.35rem 0.5rem;
    border: 1px solid rgba(247, 239, 224, 0.16);
    border-radius: 0.375rem;
    background: rgba(16, 19, 22, 0.68);
    color: #f7efe0;
    box-shadow: 0 0.5rem 1.4rem rgba(0, 0, 0, 0.16);
    font-size: 0.72rem;
    line-height: 1;
    white-space: nowrap;
    backdrop-filter: blur(10px);
  }

  .ui-frame-rate-badge span {
    color: rgba(247, 239, 224, 0.62);
    font-size: 0.66rem;
    font-weight: 700;
    text-transform: uppercase;
  }

  .ui-frame-rate-badge strong {
    font-size: 0.8rem;
    font-weight: 800;
  }

  .ui-frame-rate-badge small {
    grid-column: 2;
    color: rgba(247, 239, 224, 0.58);
    font-size: 0.64rem;
  }
</style>
