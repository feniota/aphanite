<script lang="ts">
  import Router from "svelte-spa-router";

  import Login from "../pages/Login.svelte";
  import Register from "../pages/Register.svelte";

  const routes = {
    "/": Login,
    "/register": Register,
  };
</script>

<Router {routes} />
