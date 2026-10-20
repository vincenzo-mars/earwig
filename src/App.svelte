<script lang="ts">
import { listen } from "@tauri-apps/api/event";
import { replace, router } from "svelte-spa-router";
import CompactStrip from "./lib/CompactStrip.svelte";
import Onboarding from "./lib/Onboarding.svelte";
import Recorder from "./lib/Recorder.svelte";
import RecordingDetail from "./lib/RecordingDetail.svelte";
import SettingsPanel from "./lib/SettingsPanel.svelte";
import SttIndicator from "./lib/SttIndicator.svelte";
import { servers } from "./lib/stores/servers.svelte";
import { session } from "./lib/stores/session.svelte";

$effect(() => {
  servers.refreshStt();

  const onFocus = () => {
    servers.refreshStt();
  };
  window.addEventListener("focus", onFocus);

  const unlistenSettings = listen("open-settings", () => {
    replace("/settings");
  });

  return () => {
    window.removeEventListener("focus", onFocus);
    unlistenSettings.then((fn) => fn());
  };
});

$effect(() => {
  if (typeof document !== "undefined") {
    if (session.isCompact) {
      document.documentElement.classList.add("compact-mode");
      document.body.classList.add("compact-mode");
    } else {
      document.documentElement.classList.remove("compact-mode");
      document.body.classList.remove("compact-mode");
    }
  }
});

const isSettingsOpen = $derived(router.location === "/settings");
const activeDetailId = $derived.by(() => {
  if (router.location.startsWith("/detail/")) {
    const raw = router.location.slice("/detail/".length);
    return decodeURIComponent(raw);
  }
  return null;
});

function closeModal() {
  replace("/");
}
</script>

{#if !servers.modelReady}
  <Onboarding onContinue={() => servers.refreshStt()} />
{:else if session.isCompact}
  <CompactStrip />
{:else}
<!-- La schermata base permanente è Recorder (hero REC + lista registrazioni).
     Dettaglio registrazione e impostazioni si aprono come modali sovrapposte
     sopra la home con backdrop opaco e animazione di scala. -->
<div class="relative flex h-screen flex-col overflow-hidden">
  <Recorder />

  <SttIndicator status={servers.sttStatus} />

  {#if isSettingsOpen}
    <SettingsPanel onclose={closeModal} />
  {:else if activeDetailId}
    <RecordingDetail id={activeDetailId} onclose={closeModal} />
  {/if}
</div>
{/if}
