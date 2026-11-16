// Progressive enhancement only: the site works without this file.
(() => {
  // The theme button cycles the system's, light and dark; the choice is kept and announced to the pages that draw their own pixels.
  const toggle = document.querySelector(".theme-toggle");
  if (toggle) {
    const order = ["system", "light", "dark"];
    const current = () => {
      try {
        const saved = localStorage.getItem("tapiti-theme");
        return order.includes(saved) ? saved : "system";
      } catch {
        return "system";
      }
    };
    const show = () => {
      toggle.hidden = false;
      toggle.textContent = current();
    };
    toggle.addEventListener("click", () => {
      const next = order[(order.indexOf(current()) + 1) % order.length];
      try {
        localStorage.setItem("tapiti-theme", next);
      } catch {}
      if (next === "system") delete document.documentElement.dataset.theme;
      else document.documentElement.dataset.theme = next;
      show();
      window.dispatchEvent(new Event("themechange"));
    });
    show();
  }

  // Docs: a copy button on every code block.
  const label = ["Copy", "Copied"];
  for (const pre of document.querySelectorAll(".docs-article pre")) {
    if (!navigator.clipboard) break;
    const wrap = document.createElement("div");
    wrap.className = "code-block";
    pre.replaceWith(wrap);
    wrap.append(pre);
    const button = document.createElement("button");
    button.type = "button";
    button.className = "copy";
    button.textContent = label[0];
    button.addEventListener("click", async () => {
      try {
        await navigator.clipboard.writeText(pre.innerText);
        button.textContent = label[1];
        setTimeout(() => (button.textContent = label[0]), 1600);
      } catch {}
    });
    wrap.append(button);
  }

  // Docs: the table of contents follows the section being read.
  const links = [...document.querySelectorAll(".docs-toc a")];
  if (links.length && "IntersectionObserver" in window) {
    const byId = new Map(links.map((a) => [decodeURIComponent(a.hash.slice(1)), a]));
    const heads = [...byId.keys()].map((id) => document.getElementById(id)).filter(Boolean);
    const seen = new Set();
    const mark = () => {
      const first = heads.find((h) => seen.has(h));
      if (!first) return;
      for (const a of links) a.removeAttribute("aria-current");
      byId.get(first.id)?.setAttribute("aria-current", "location");
    };
    const io = new IntersectionObserver(
      (entries) => {
        for (const e of entries) (e.isIntersecting ? seen.add(e.target) : seen.delete(e.target));
        mark();
      },
      { rootMargin: `-${parseInt(getComputedStyle(document.documentElement).getPropertyValue("--header-h")) + 16 || 80}px 0px -60% 0px` },
    );
    heads.forEach((h) => io.observe(h));
  }
})();
