<script lang="ts">
  import type { Component, Snippet } from "svelte";

  let { icon: Icon, title, children }: { icon: Component<any>; title: string; children?: Snippet } = $props();
</script>

<div class="empty">
  <div class="badge"><Icon size={22} strokeWidth={1.6} /></div>
  <h3>{title}</h3>
  {#if children}<p>{@render children()}</p>{/if}
</div>

<style>
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    padding: 56px 24px;
    animation: fade 0.3s var(--ease);
  }
  .badge {
    display: grid;
    place-items: center;
    width: 48px;
    height: 48px;
    margin-bottom: 14px;
    border-radius: var(--r-lg);
    background: var(--surface-1);
    border: 1px solid var(--hairline);
    color: var(--ink-subtle);
    box-shadow: var(--edge-highlight);
  }
  h3 {
    margin: 0 0 6px;
    font-size: 15px;
    font-weight: 600;
    letter-spacing: -0.01em;
  }
  p {
    margin: 0;
    max-width: 340px;
    font-size: 13px;
    line-height: 1.6;
    color: var(--ink-subtle);
  }
  p :global(b) {
    color: var(--ink-muted);
    font-weight: 500;
  }
  @keyframes fade {
    from {
      opacity: 0;
      transform: translateY(4px);
    }
  }
</style>
