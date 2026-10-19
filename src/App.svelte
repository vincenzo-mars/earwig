<script lang="ts">
import Router, { router } from "svelte-spa-router";
import Onboarding from "./lib/Onboarding.svelte";
import { routes } from "./lib/routes";
import SttIndicator from "./lib/SttIndicator.svelte";
import { servers } from "./lib/stores/servers.svelte";

$effect(() => {
  servers.refreshStt();
  servers.refreshLlm();
});
</script>

{#if !servers.modelReady}
  <Onboarding onContinue={() => servers.refreshStt()} />
{:else}
<!-- Nessun padding e nessun centraggio qui: ogni rotta decide la propria
     larghezza, e le pagine interne arrivano fino al bordo della finestra.
     `overflow-hidden` tiene lo scroll dentro la rotta, così l'header di
     pagina può restare sticky in cima. -->
<div class="flex h-screen flex-col overflow-hidden">
  <Router {routes} />

<!-- La pillola vive su ogni rotta tranne /settings: lì lo stato del server
     è già dentro le sezioni ServerControls, duplicarlo a schermo è rumore. -->
{#if router.location !== "/settings"}
  <SttIndicator status={servers.sttStatus} />
{/if}
</div>
{/if}
