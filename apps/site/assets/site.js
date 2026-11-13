// Progressive enhancement only: the site works without this file.
(() => {
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

  // The entrance: show the install that fits the visitor's system, and let the Linux command be copied. Unknown or phone: all three stay.
  const install = document.querySelector("[data-install]");
  if (install) {
    const ua = navigator.userAgentData?.platform || navigator.platform || "";
    const agent = navigator.userAgent;
    const mobile = /Android|iPhone|iPad|iPod/i.test(agent) || navigator.userAgentData?.mobile;
    const os = mobile ? null : /win/i.test(ua) ? "windows" : /mac/i.test(ua) ? "macos" : /linux|x11/i.test(ua) || /Linux|X11/.test(agent) ? "linux" : null;
    const pick = install.querySelector(`[data-os="${os}"]`);
    if (pick) {
      pick.dataset.picked = "";
      install.dataset.picked = "";
    }
    const copy = install.querySelector(".copy-command");
    if (copy && navigator.clipboard) {
      copy.hidden = false;
      const text = copy.textContent;
      copy.addEventListener("click", async () => {
        try {
          await navigator.clipboard.writeText(copy.dataset.copy);
          copy.textContent = copy.dataset.copied;
          setTimeout(() => (copy.textContent = text), 1600);
        } catch {}
      });
    }
  }
})();
