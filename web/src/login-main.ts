import { mount } from "svelte";

import "./login.css";
import AuthRouter from "./lib/AuthRouter.svelte";

const app = mount(AuthRouter, {
  target: document.getElementById("app")!,
});

export default app;
