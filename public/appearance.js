(() => {
  const key = 'legura-appearance-mode';
  const valid = value => value === 'light' || value === 'dark';
  let mode = 'light';
  try {
    const saved = localStorage.getItem(key);
    if (valid(saved)) mode = saved;
  } catch {
    // Restricted storage must not prevent using the current page.
  }

  function apply() {
    document.querySelectorAll('.legura-app').forEach(node => {
      node.dataset.leguraJs = 'true';
      if (node.dataset.foundryMode !== mode) node.dataset.foundryMode = mode;
    });
    document.querySelectorAll('[data-legura-mode-picker]').forEach(control => {
      control.value = mode;
      control.closest('label').hidden = false;
    });
  }

  // This blocking head script observes parser insertions before the first paint.
  // Keeping the observer also covers page fragments replaced by the state bridge.
  new MutationObserver(apply).observe(document.documentElement, { childList: true, subtree: true });
  apply();
  document.addEventListener('change', event => {
    if (!event.target.matches('[data-legura-mode-picker]') || !valid(event.target.value)) return;
    mode = event.target.value;
    try { localStorage.setItem(key, mode); } catch { /* In-memory preference still works. */ }
    apply();
  });
  window.addEventListener('storage', event => {
    if (event.key !== key && event.key !== null) return;
    mode = valid(event.newValue) ? event.newValue : 'light';
    apply();
  });
})();
