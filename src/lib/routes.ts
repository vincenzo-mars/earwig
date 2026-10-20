import type { RouteDefinition } from "svelte-spa-router";
import Recorder from "./Recorder.svelte";
import RecordingDetail from "./RecordingDetail.svelte";
import SettingsPanel from "./SettingsPanel.svelte";

// Hash routing sotto Tauri: l'applicazione desktop adotta una schermata
// base permanente (`Recorder`: hero REC + lista registrazioni).
// Le schermate di dettaglio registrazione (`/detail/:id`) e impostazioni (`/settings`)
// sono modali sincronizzate con l'hash di svelte-spa-router, visualizzate come
// overlay con backdrop sfocato e controlli di chiusura dedicati (X, Escape, click esterno).
export const routes: RouteDefinition = {
  "/": Recorder,
  "/detail/:id": RecordingDetail,
  "/settings": SettingsPanel,
  "*": Recorder,
};
