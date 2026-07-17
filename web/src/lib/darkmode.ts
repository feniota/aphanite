function init() {
  const dm = localStorage.getItem("aphanite.dark-mode");
  const media = window.matchMedia("(prefers-color-scheme: dark)");
  switch (dm) {
    case "dark":
      toggle();
      break;
    case "system":
      if (media.matches) {
        toggle();
      }
      break;
    default:
      if (dm === "light") break;
      else localStorage.setItem("aphanite.dark-mode", "system");
      init();
      return;
  }

  media.addEventListener("change", ()=>{
    if(localStorage.getItem("aphanite.dark-mode")==="system") toggle();
  })
}

init();

export function toggle() {
  window.document.body.classList.toggle("dark");
}
