export type DarkMode = "light" | "dark" | "system";

export function get_dark_mode(): boolean {
  const dm = localStorage.getItem("aphanite.dark-mode");
  if (dm === "dark" || dm === "light") return dm === "dark";
  else {
    if (dm === null) {
      localStorage.setItem("aphanite.dark-mode", "system");
    }
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    return media.matches;
  }
}

export function set_dark_mode(mode: DarkMode) {
  localStorage.setItem("aphanite.dark-mode", mode);
  switch (mode) {
    case "dark":
      document.body.classList.add("dark");
      break;
    case "light":
      document.body.classList.remove("dark");
      break;
    default: {
      if (get_dark_mode()) {
        document.body.classList.add("dark");
      } else {
        document.body.classList.remove("dark");
      }
    }
  }
}

function init() {
  const dark_mode = get_dark_mode();
  const media = window.matchMedia("(prefers-color-scheme: dark)");
  if (dark_mode) {
    document.body.classList.add("dark");
  }
  media.addEventListener("change", e => {
    if (localStorage.getItem("aphanite.dark-mode") === "system") {
      if (e.matches) {
        document.body.classList.add("dark");
      } else {
        document.body.classList.remove("dark");
      }
    }
  });
}

init();
