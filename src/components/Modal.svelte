<script lang="ts">
  import type { Snippet } from "svelte";
  import { portal } from "../lib/portal";
  import { X } from "../lib/icons";

  let { title, onclose, children, width = 440 }: { title: string; onclose: () => void; children: Snippet; width?: number } =
    $props();
</script>

<svelte:window
  onkeydown={(e) => {
    if (e.key === "Escape") {
      e.stopPropagation();
      onclose();
    }
  }}
/>

<div class="scrim" use:portal role="presentation" onpointerdown={(e) => e.target === e.currentTarget && onclose()}>
  <div class="modal" role="dialog" aria-modal="true" aria-label={title} style:width="{width}px">
    <div class="head">
      <h2>{title}</h2>
      <button type="button" class="icon-btn" onclick={onclose} aria-label="Fechar"><X size={17} /></button>
    </div>
    {@render children()}
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 900;
    display: grid;
    place-items: center;
    padding: 24px;
    background: var(--scrim);
    animation: fade 0.15s ease;
  }
  .modal {
    max-width: 100%;
    max-height: calc(100vh - 48px);
    overflow: auto;
    background: var(--surface-pop);
    border-radius: var(--r-lg);
    box-shadow: var(--shadow-pop);
    animation: rise 0.18s var(--ease);
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 14px 4px 22px;
  }
  h2 {
    margin: 0;
    font-size: 16px;
    font-weight: 600;
    letter-spacing: -0.2px;
  }
  @keyframes fade {
    from {
      opacity: 0;
    }
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(8px) scale(0.985);
    }
  }
</style>
