<script lang="ts">
  import type { Snippet } from "svelte";
  import { portal } from "../lib/portal";

  let {
    anchor,
    open = $bindable(false),
    align = "start",
    width,
    children,
    onclose,
  }: {
    anchor: HTMLElement | undefined;
    open?: boolean;
    align?: "start" | "end";
    width?: number;
    children: Snippet;
    onclose?: () => void;
  } = $props();

  let el: HTMLDivElement | undefined = $state();
  let pos = $state({ top: 0, left: 0, placed: false });

  function place() {
    if (!anchor || !el) return;
    const a = anchor.getBoundingClientRect();
    const r = el.getBoundingClientRect();
    let left = align === "end" ? a.right - r.width : a.left;
    let top = a.bottom + 6;
    if (top + r.height > innerHeight - 8) top = Math.max(8, a.top - r.height - 6);
    left = Math.min(Math.max(8, left), innerWidth - r.width - 8);
    pos = { top, left, placed: true };
  }

  function close() {
    open = false;
    onclose?.();
  }

  $effect(() => {
    if (!open || !el) return;
    place();
    const onDown = (e: PointerEvent) => {
      const t = e.target as Node;
      if (!el?.contains(t) && !anchor?.contains(t)) close();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        e.preventDefault();
        close();
      }
    };
    document.addEventListener("pointerdown", onDown, true);
    document.addEventListener("keydown", onKey, true);
    window.addEventListener("resize", place);
    return () => {
      document.removeEventListener("pointerdown", onDown, true);
      document.removeEventListener("keydown", onKey, true);
      window.removeEventListener("resize", place);
    };
  });

  $effect(() => {
    if (!open) pos.placed = false;
  });
</script>

{#if open}
  <div
    class="pop"
    bind:this={el}
    use:portal
    role="dialog"
    style:top="{pos.top}px"
    style:left="{pos.left}px"
    style:width={width ? `${width}px` : undefined}
    style:visibility={pos.placed ? "visible" : "hidden"}
  >
    {@render children()}
  </div>
{/if}

<style>
  .pop {
    position: fixed;
    z-index: 1000;
    min-width: 180px;
    max-height: calc(100vh - 16px);
    overflow: auto;
    padding: 4px;
    background: var(--surface-pop);
    border-radius: var(--r-lg);
    box-shadow: var(--shadow-pop);
    animation: pop-in 0.12s var(--ease);
    transform-origin: top left;
  }
  @keyframes pop-in {
    from {
      opacity: 0;
      transform: translateY(-4px) scale(0.98);
    }
  }
</style>
