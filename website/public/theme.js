// Loaded synchronously in <head> so the saved theme applies before first paint.
(() => {
  const KEY = "zinc-oxide-theme";
  const THEMES = ["system", "zinc", "nord", "gruvbox", "paper", "solarized"];
  const root = document.documentElement;

  const load = () => {
    try {
      const saved = localStorage.getItem(KEY);
      return THEMES.includes(saved) ? saved : "system";
    } catch {
      return "system";
    }
  };

  const save = (theme) => {
    try {
      localStorage.setItem(KEY, theme);
    } catch {
      // Storage can be blocked (private mode); the choice just won't persist.
    }
  };

  const apply = (theme) => {
    if (theme === "system") {
      root.removeAttribute("data-theme");
    } else {
      root.setAttribute("data-theme", theme);
    }
  };

  apply(load());

  document.addEventListener("DOMContentLoaded", () => {
    const select = document.getElementById("theme-select");
    if (!select) return;
    select.value = load();
    select.addEventListener("change", () => {
      apply(select.value);
      save(select.value);
    });
  });
})();
