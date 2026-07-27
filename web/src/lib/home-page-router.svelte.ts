import { type Component } from "svelte";

import { t } from "@/lib/i18n.svelte";
import Home from "@/pages/Home.svelte";
import PlayerProfileDetails from "@/pages/PlayerProfileDetails.svelte";
import PlayerProfiles from "@/pages/PlayerProfiles.svelte";
import User from "@/pages/User.svelte";

const ROUTE_DEFS: {
  path: string;
  component: Component;
  title_key: string;
  hidden?: boolean;
}[] = [
  {
    path: "/",
    component: Home,
    title_key: "sidebar.home",
  },
  {
    path: "/profiles",
    component: PlayerProfiles,
    title_key: "sidebar.profiles",
  },
  {
    path: "/profile/:id",
    component: PlayerProfileDetails,
    title_key: "sidebar.profile_details",
    hidden: true,
  },
  {
    path: "/user",
    component: User,
    title_key: "sidebar.user",
  },
];

export function routes_with_title(): {
  path: string;
  component: Component;
  title: string;
  hidden?: boolean;
}[] {
  return ROUTE_DEFS.map(r => ({
    path: r.path,
    component: r.component,
    title: t(r.title_key),
    hidden: r.hidden,
  }));
}

export const routes: { [x: string]: Component } = (() => {
  const ret: { [x: string]: Component } = {};
  for (const route of ROUTE_DEFS) {
    ret[route.path] = route.component;
  }
  return ret;
})();
