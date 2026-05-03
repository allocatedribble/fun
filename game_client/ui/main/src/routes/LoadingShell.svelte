<script lang="ts">
</script>

<section class="loading-shell" aria-live="polite">
  <div class="loading-mark"></div>
  <strong>Fun Client</strong>
  <span>Starting host services</span>
</section>

<style>
  .loading-shell {
    min-height: 100vh;
    display: grid;
    place-content: center;
    gap: 0.75rem;
    background: #101316;
    color: #f7efe0;
    text-align: center;
  }

  .loading-mark {
    width: 2.25rem;
    aspect-ratio: 1;
    border: 2px solid rgba(247, 239, 224, 0.18);
    border-top-color: #d7a85b;
    border-radius: 50%;
    justify-self: center;
    animation: spin 900ms linear infinite;
  }

  .loading-shell span {
    color: rgba(247, 239, 224, 0.68);
    font-size: 0.85rem;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
