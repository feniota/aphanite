import { mount } from "svelte";

import "./app.css";
import { init_i18n } from "@/lib/i18n.svelte.ts";
import NotFound from "@/pages/NotFound.svelte";

await init_i18n();

const app = mount(NotFound, {
  target: document.getElementById("app")!,
});

export default app;
