// Runs in the head, before the page is painted: puts the theme the visitor chose last time on the page, so a dark system with a light
// choice does not flash. With no choice the page follows the system (the stylesheet decides). Progressive enhancement only.
(() => {
  try {
    const choice = localStorage.getItem("tapiti-theme");
    if (choice === "light" || choice === "dark") document.documentElement.dataset.theme = choice;
  } catch {}
})();
