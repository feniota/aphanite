import { mount } from "svelte";

import "./app.css";
import { init_i18n } from "@/lib/i18n.svelte.ts";

import Router from "./components/LoginPageRouter.svelte";

await init_i18n();

const app = mount(Router, {
  target: document.getElementById("app")!,
});

export default app;
