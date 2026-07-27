import { mount } from "svelte";

import "./app.css";
import { init_i18n } from "@/lib/i18n.svelte";

import App from "./App.svelte";

await init_i18n();

const app = mount(App, {
  target: document.getElementById("app")!,
});

export default app;
