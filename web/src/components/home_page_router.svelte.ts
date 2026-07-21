import { type Component } from "svelte";

import Home from "@/pages/Home.svelte";

/** Routes used in Aphanite homepage */
export const routes_with_title: { path: string; component: Component; title: string }[] = [
  {
    path: "/",
    component: Home,
    title: "首页",
  },
];

/** De-titled `routes_with_title` for passing into svelte-spa-router */
export const routes: { [x: string]: Component } = (() => {
  const ret: { [x: string]: Component } = {};
  for (const route of routes_with_title) {
    ret[route.path] = route.component;
  }
  return ret;
})();
