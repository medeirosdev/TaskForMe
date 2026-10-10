/** Move o elemento para o <body>, para popovers nunca serem cortados por overflow. */
export function portal(node: HTMLElement) {
  document.body.appendChild(node);
  return {
    destroy() {
      node.remove();
    },
  };
}

/** Chama `cb` ao clicar fora do elemento (e fora de `ignore`, se informado). */
export function clickOutside(node: HTMLElement, cb: () => void) {
  const handler = (e: PointerEvent) => {
    if (!node.contains(e.target as Node)) cb();
  };
  // Adia para não capturar o próprio clique que abriu o popover.
  const t = setTimeout(() => document.addEventListener("pointerdown", handler, true));
  return {
    destroy() {
      clearTimeout(t);
      document.removeEventListener("pointerdown", handler, true);
    },
  };
}

/** Textarea que cresce com o conteúdo. */
export function autosize(node: HTMLTextAreaElement, _value?: unknown) {
  const fit = () => {
    node.style.height = "auto";
    node.style.height = `${node.scrollHeight}px`;
  };
  node.addEventListener("input", fit);
  requestAnimationFrame(fit);
  return {
    update() {
      requestAnimationFrame(fit);
    },
    destroy() {
      node.removeEventListener("input", fit);
    },
  };
}
