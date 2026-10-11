<script lang="ts">
  import SectionHeader from "./SectionHeader.svelte";
  import EmptyState from "./EmptyState.svelte";
  import { Copy, LinkIcon, Pencil, Plus, Trash } from "../lib/icons";
  import { api } from "../lib/api";
  import { app, deleteWithUndo } from "../lib/store.svelte";
  import { toast } from "../lib/toast.svelte";
  import type { Link, Project } from "../lib/types";

  let { project }: { project: Project } = $props();

  let links = $state<Link[]>([]);
  let loaded = $state(false);

  $effect(() => {
    const id = project.id;
    void app.version;
    api
      .linksByProject(id)
      .then((r) => {
        if (id !== project.id) return;
        links = r;
        loaded = true;
      })
      .catch(() => {});
  });

  // Ctrl+N na aba Links.
  $effect(() => {
    const onNew = () => add(null);
    window.addEventListener("tfm:new-link", onNew);
    return () => window.removeEventListener("tfm:new-link", onNew);
  });

  const sections = $derived(app.sectionsOf(project.id));
  const inSection = (sid: number | null) => links.filter((l) => l.sectionId === sid);

  function add(sectionId: number | null) {
    app.linkDialog = { projectId: project.id, sectionId };
  }

  function domain(url: string) {
    try {
      return new URL(url).hostname.replace(/^www\./, "");
    } catch {
      return url;
    }
  }

  /** Cor estável por domínio, da paleta de projetos. */
  function tint(url: string) {
    const d = domain(url);
    let h = 0;
    for (const c of d) h = (h * 31 + c.charCodeAt(0)) >>> 0;
    const palette = ["#5e6ad2", "#4ea7fc", "#26b5ce", "#4cb782", "#f2994a", "#eb5757", "#e46ead", "#9b8afb"];
    return palette[h % palette.length];
  }

  async function copy(l: Link) {
    try {
      await navigator.clipboard.writeText(l.url);
      toast("Link copiado");
    } catch {
      toast("Não foi possível copiar o link", { kind: "error" });
    }
  }
</script>

{#snippet grid(items: Link[])}
  {#if items.length}
    <div class="grid">
      {#each items as l (l.id)}
        <div class="card">
          <button type="button" class="open" title="Abrir {l.url}" onclick={() => api.openUrl(l.url).catch(() => {})}>
            <span class="avatar" style:--tint={tint(l.url)}>{domain(l.url).charAt(0).toUpperCase()}</span>
            <span class="text">
              <span class="title">{l.title}</span>
              {#if l.description}<span class="desc">{l.description}</span>{/if}
              <span class="domain">{domain(l.url)}</span>
            </span>
          </button>
          <div class="actions">
            <button type="button" class="icon-btn sm" title="Copiar endereço" onclick={() => copy(l)}>
              <Copy size={14} />
            </button>
            <button
              type="button"
              class="icon-btn sm"
              title="Editar"
              onclick={() => (app.linkDialog = { projectId: l.projectId, sectionId: l.sectionId, link: l })}
            >
              <Pencil size={14} />
            </button>
            <button
              type="button"
              class="icon-btn sm"
              title="Excluir"
              onclick={() => deleteWithUndo(`Link "${l.title}" excluído`, () => api.linkDelete(l.id))}
            >
              <Trash size={14} />
            </button>
          </div>
        </div>
      {/each}
    </div>
  {/if}
{/snippet}

<div class="wrap">
  <button type="button" class="new" onclick={() => add(null)}>
    <Plus size={15} strokeWidth={2} /> Novo link
  </button>

  {@render grid(inSection(null))}

  {#each sections as s (s.id)}
    <SectionHeader section={s} count={inSection(s.id).length} onadd={() => add(s.id)} addLabel="Novo link" />
    {#if !s.collapsed}
      {@render grid(inSection(s.id))}
      {#if !inSection(s.id).length}
        <button type="button" class="add-row" onclick={() => add(s.id)}>+ Adicionar link</button>
      {/if}
    {/if}
  {/each}

  {#if loaded && links.length === 0 && sections.length === 0}
    <EmptyState icon={LinkIcon} title="Nenhum link ainda">
      Guarde aqui os sites que você usa neste projeto: sistemas, documentos, painéis. Um clique abre no
      navegador.
    </EmptyState>
  {/if}
</div>

<style>
  .wrap {
    display: flex;
    flex-direction: column;
    padding-bottom: 48px;
  }
  .new {
    align-self: flex-start;
    display: flex;
    align-items: center;
    gap: 8px;
    height: 36px;
    padding: 0 14px;
    border-radius: var(--r-md);
    font-size: 13px;
    font-weight: 500;
    color: var(--primary-ink);
    border: 1px dashed var(--hairline-strong);
  }
  .new:hover {
    background: var(--primary-soft);
    border-color: var(--primary);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
    gap: 10px;
    margin-top: 12px;
  }
  .card {
    position: relative;
    border-radius: var(--r-lg);
    background: var(--surface-1);
    border: 1px solid var(--hairline);
    box-shadow: var(--edge-highlight);
    transition: border-color 0.12s, background 0.12s;
  }
  .card:hover {
    border-color: var(--hairline-strong);
    background: var(--surface-2);
  }
  .open {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    width: 100%;
    padding: 14px 14px 14px 14px;
    text-align: left;
    border-radius: var(--r-lg);
  }
  .avatar {
    display: grid;
    place-items: center;
    width: 34px;
    height: 34px;
    flex-shrink: 0;
    border-radius: var(--r-md);
    font-size: 15px;
    font-weight: 600;
    color: var(--tint);
    background: color-mix(in srgb, var(--tint) 16%, transparent);
  }
  .text {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
    padding-right: 64px;
  }
  .title {
    font-size: 14px;
    font-weight: 500;
    color: var(--ink);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .desc {
    font-size: 12.5px;
    line-height: 1.45;
    color: var(--ink-subtle);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .domain {
    font-size: 11.5px;
    color: var(--ink-tertiary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .actions {
    position: absolute;
    top: 8px;
    right: 8px;
    display: flex;
    gap: 1px;
    opacity: 0;
    transition: opacity 0.12s;
  }
  .card:hover .actions,
  .actions:focus-within {
    opacity: 1;
  }
  .add-row {
    align-self: flex-start;
    height: 32px;
    margin-top: 4px;
    padding: 0 10px 0 28px;
    font-size: 13px;
    color: var(--ink-tertiary);
  }
  .add-row:hover {
    color: var(--primary-ink);
  }
</style>
