export function disableBrowserContextMenu(): void {
  if (import.meta.env.DEV) return;

  document.addEventListener("contextmenu", (event) => {
    const target = event.target as HTMLElement | null;
    if (target?.closest("input, textarea, [contenteditable='true']")) return;
    if (!window.getSelection()?.isCollapsed) return;
    event.preventDefault();
  });
}
