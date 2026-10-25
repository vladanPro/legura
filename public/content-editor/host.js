// Application lifecycle glue; editing and document validation live in the package.
import { mountRichTextEditor } from '/content-editor/60e28caa76c3edae1e513847b9530f4f0fb17f987a88b422132fbb521600e5fc/rich-text-editor.js';

const mounted = new Map();
const observer = new MutationObserver(reconcile);
function reconcile() {
  for (const [root, handle] of mounted) {
    if (!root.isConnected) { handle.destroy(); mounted.delete(root); }
  }
  for (const root of document.querySelectorAll('[data-content-editor-form] [data-legura-editor="richtext"]')) {
    if (!mounted.has(root)) mounted.set(root, mountRichTextEditor(root));
    const save = root.closest('form').querySelector('[data-editor-save]');
    const disabled = root.querySelector('[data-editor-document]').disabled;
    if (save.disabled !== disabled) save.disabled = disabled;
  }
}
function start() {
  reconcile();
  observer.observe(document.body, { childList: true, subtree: true, attributes: true, attributeFilter: ['disabled'] });
}
window.addEventListener('pagehide', () => {
  observer.disconnect();
  for (const [root, handle] of mounted) {
    const field = root.querySelector('[data-editor-document]');
    if (!field.disabled) root.dataset.document = field.value;
    handle.destroy();
  }
  mounted.clear();
});
window.addEventListener('pageshow', event => { if (event.persisted) start(); });
start();
