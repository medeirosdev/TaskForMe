<script lang="ts">
  import { CircleAlert, X } from "../lib/icons";
  import { dismiss, toasts } from "../lib/toast.svelte";
</script>

<div class="stack" aria-live="polite">
  {#each toasts as t (t.id)}
    <div class="toast" class:error={t.kind === "error"}>
      {#if t.kind === "error"}<CircleAlert size={16} />{/if}
      <span class="msg">{t.message}</span>
      {#if t.action}
        <button type="button"
          class="action"
          onclick={() => {
            t.action!.run();
            dismiss(t.id);
          }}>{t.action.label}</button
        >
      {/if}
      <button type="button" class="close" aria-label="Fechar" onclick={() => dismiss(t.id)}><X size={14} /></button>
    </div>
  {/each}
</div>

<style>
  .stack {
    position: fixed;
    left: 50%;
    bottom: 20px;
    transform: translateX(-50%);
    z-index: 1100;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    pointer-events: none;
  }
  .toast {
    display: flex;
    align-items: center;
    gap: 10px;
    max-width: min(560px, calc(100vw - 40px));
    min-height: 40px;
    padding: 8px 8px 8px 14px;
    border-radius: var(--r-md);
    background: var(--surface-pop);
    box-shadow: var(--shadow-pop);
    font-size: 13px;
    color: var(--ink);
    pointer-events: auto;
    animation: up 0.22s var(--ease);
  }
  .toast.error {
    color: var(--danger);
  }
  .msg {
    flex: 1;
    line-height: 1.45;
  }
  .toast.error .msg {
    color: var(--ink);
  }
  .action {
    font-weight: 600;
    color: var(--primary-ink);
    padding: 4px 8px;
    border-radius: var(--r-sm);
    white-space: nowrap;
  }
  .action:hover {
    background: var(--primary-soft);
  }
  .close {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border-radius: var(--r-sm);
    color: var(--ink-tertiary);
  }
  .close:hover {
    background: var(--hover);
    color: var(--ink);
  }
  @keyframes up {
    from {
      opacity: 0;
      transform: translateY(10px);
    }
  }
</style>
