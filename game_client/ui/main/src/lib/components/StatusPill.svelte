<script lang="ts">
  export let label: string;
  export let value: string;
  export let tone: 'neutral' | 'good' | 'warn' | 'bad' = 'neutral';
</script>

<span class={`status-pill ${tone}`}>
  <span>{label}</span>
  <strong>{value}</strong>
</span>
