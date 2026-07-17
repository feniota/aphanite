import { mount } from "svelte";

import "./app.css";
import AuthRouter from "./components/AuthRouter.svelte";

const app = mount(AuthRouter, {
  target: document.getElementById("app")!,
});

export default app;
