import { mount } from "svelte";

import "./app.css";
import Router from "./components/LoginPageRouter.svelte";

const app = mount(Router, {
  target: document.getElementById("app")!,
});

export default app;
