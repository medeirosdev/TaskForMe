<script lang="ts">
  import { Check } from "../lib/icons";

  let {
    checked,
    onchange,
    size = 18,
    color,
    label = "Concluir",
  }: { checked: boolean; onchange: (v: boolean) => void; size?: number; color?: string; label?: string } = $props();
</script>

<button type="button"
  class="cb"
  class:checked
  role="checkbox"
  aria-checked={checked}
  aria-label={label}
  style:--size="{size}px"
  style:--ring={color ?? "var(--ink-tertiary)"}
  onclick={(e) => {
    e.stopPropagation();
    onchange(!checked);
  }}
>
  <span class="box">
    {#if checked}<Check size={size - 6} strokeWidth={3} />{/if}
  </span>
</button>

<style>
  .cb {
    display: grid;
    place-items: center;
    width: calc(var(--size) + 8px);
    height: calc(var(--size) + 8px);
    margin: -4px;
    border-radius: 50%;
    flex-shrink: 0;
  }
  .box {
    display: grid;
    place-items: center;
    width: var(--size);
    height: var(--size);
    border-radius: 50%;
    border: 1.5px solid var(--ring);
    color: var(--on-primary);
    transition: background 0.15s, border-color 0.15s, transform 0.15s var(--ease);
  }
  .cb:hover .box {
    border-color: var(--primary);
    background: var(--primary-soft);
  }
  .checked .box,
  .cb.checked:hover .box {
    background: var(--primary);
    border-color: var(--primary);
    animation: tick 0.28s var(--ease);
  }
  @keyframes tick {
    40% {
      transform: scale(1.18);
    }
  }
</style>
