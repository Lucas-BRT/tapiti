// Runs in the head, before the page is painted: puts the choices the visitor made last time on the page (theme, text size, motion), so a
// dark system with a light choice does not flash and the text does not jump. With no choice the page follows the system (the stylesheet
// decides). Progressive enhancement only; settings.js owns the rules and runs again once the page is loaded.
(() => {
  try {
    const html = document.documentElement;
    const s = JSON.parse(localStorage.getItem("tapiti.settings") || "{}");
    const theme = s.theme || localStorage.getItem("tapiti-theme");
    if (theme === "light" || theme === "dark") html.dataset.theme = theme;
    const text = { small: 90, normal: 100, large: 115, larger: 130 };
    const pct = s.text ? (s.text.preset === "custom" ? s.text.value : text[s.text.preset]) : 100;
    if (pct && pct !== 100) html.style.zoom = String(pct / 100);
    const motion = { off: 0, fast: 80, normal: 120, slow: 220 };
    const m = s.motion;
    const ms = m && m.preset ? (m.preset === "custom" ? m.value : motion[m.preset]) : matchMedia("(prefers-reduced-motion: reduce)").matches ? 0 : 120;
    html.style.setProperty("--t", ms + "ms");
    html.dataset.motion = ms === 0 ? "off" : "on";
  } catch {}
})();
