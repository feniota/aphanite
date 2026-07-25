import { type Component } from "svelte";

import Home from "@/pages/Home.svelte";
import PlayerProfileDetails from "@/pages/PlayerProfileDetails.svelte";
import PlayerProfiles from "@/pages/PlayerProfiles.svelte";

/** Routes used in Aphanite homepage */
export const routes_with_title: {
  path: string;
  component: Component;
  title: string;
  /** Whether this should not appear in the sidebar */
  hidden?: boolean;
}[] = [
  {
    path: "/",
    component: Home,
    title: "首页",
  },
  {
    path: "/profiles",
    component: PlayerProfiles,
    title: "玩家档案",
  },
  {
    path: "/profile/:id",
    component: PlayerProfileDetails,
    title: "玩家档案信息",
    hidden: true,
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
