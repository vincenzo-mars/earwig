<script lang="ts">
import { GripVertical, Maximize2, Play, Square } from "@lucide/svelte";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { recordings } from "./stores/recordings.svelte";
import { session } from "./stores/session.svelte";
import { cn } from "./utils";

let stage = $state<"idle" | "expanded">("idle");
let isDragging = $state(false);
let dragStartY = 0;
let windowStartY = 0;

// Ascolta gli eventi di cambio stato emessi dal backend
$effect(() => {
  const unlisten = listen<string>("compact-stage", (event) => {
    if (isDragging) return;
    if (event.payload === "idle" || event.payload === "expanded") {
      stage = event.payload;
    }
  });

  return () => {
    unlisten.then((fn) => fn());
  };
});

async function handleMouseEnter() {
  if (isDragging) return;
  stage = "expanded";
  try {
    await invoke("set_compact_hover", {
      hovered: true,
      isRecording: session.isRecording,
    });
  } catch (e) {
    console.error("Failed to update compact hover:", e);
  }
}

async function handleMouseLeave() {
  if (isDragging) return;
  stage = "idle";
  try {
    await invoke("set_compact_hover", {
      hovered: false,
      isRecording: session.isRecording,
    });
  } catch (e) {
    console.error("Failed to collapse compact window:", e);
  }
}

// Sincronizza la dimensione se cambia lo stato di registrazione mentre è aperta
$effect(() => {
  const rec = session.isRecording;
  if (stage === "expanded") {
    invoke("set_compact_hover", {
      hovered: true,
      isRecording: rec,
    }).catch(() => {});
  }
});

async function handleRecordToggle() {
  if (session.isRecording) {
    await session.stopRecording();
    await recordings.load(true);
  } else {
    await session.startRecording();
  }
}

async function handleGripPointerDown(e: PointerEvent) {
  e.preventDefault();
  isDragging = true;
  dragStartY = e.screenY;

  try {
    const pos = await invoke<[number, number]>("get_compact_position");
    windowStartY = pos[1];
  } catch {
    windowStartY = e.screenY - 24;
  }

  const onPointerMove = (ev: PointerEvent) => {
    if (!isDragging) return;
    const deltaY = ev.screenY - dragStartY;
    invoke("set_compact_y", { y: windowStartY + deltaY }).catch(() => {});
  };

  const onPointerUp = (ev: PointerEvent) => {
    isDragging = false;
    window.removeEventListener("pointermove", onPointerMove);
    window.removeEventListener("pointerup", onPointerUp);
    if (
      ev.clientX < 0 ||
      ev.clientX > window.innerWidth ||
      ev.clientY < 0 ||
      ev.clientY > window.innerHeight
    ) {
      handleMouseLeave();
    }
  };

  window.addEventListener("pointermove", onPointerMove);
  window.addEventListener("pointerup", onPointerUp);
}
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="relative flex h-full w-full select-none items-center justify-end overflow-hidden bg-transparent pr-0"
  onmouseenter={handleMouseEnter}
  onmouseleave={handleMouseLeave}
>
  <!--
    Linguetta Mac Premium ancorata a destra (h-10 / 40px, rounded-l-2xl).
    I 2 stati:
    1. Idle (non hover): w-[12px], linguetta iniziale aumentata con sobrio micro-dot.
    2. Expanded (hover): w-[82px], ultra-compatta senza timer, controlli completi.
  -->
  <div
    class={cn(
      "absolute right-0 flex h-10 items-center overflow-hidden rounded-l-2xl border-y border-l bg-brand-darker/95 backdrop-blur-md shadow-raised transition-all duration-200 ease-[cubic-bezier(0.16,1,0.3,1)]",
      session.isRecording ? "border-danger/50" : "border-brand-cream/15",
      stage === "idle" && "w-[12px] cursor-pointer",
      stage === "expanded" && "w-[82px]",
    )}
    title={stage === "idle"
      ? session.isRecording
        ? "Registrazione attiva"
        : "Earwig"
      : undefined}
  >
    {#if stage === "idle"}
      <!-- ── STATO A RIPOSO: LINGUETTA INIZIALE AUMENTATA (SOBRIA, MAC PREMIUM) ── -->
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <div
        class="flex h-full w-full items-center justify-start pl-1 cursor-pointer"
        onclick={handleMouseEnter}
      >
        <span
          class={cn(
            "rounded-full block",
            session.isRecording ? "size-1.5 bg-rec-strong" : "size-1.5 bg-rec/85",
          )}
        ></span>
      </div>
    {:else}
      <!-- ── STATO HOVER: CONTROLLI COMPATTI CON SPAZI OTTIMIZZATI ── -->
      <div class="flex h-full w-full items-center justify-between pl-1 pr-0.5 gap-0">
        <!-- Bottone circolare REC / STOP (senza timer, spazio bordo ridotto) -->
        {#if session.isRecording}
          <!-- Bezel registrazione circolare (identico al tasto play) -->
          <div
            class="relative flex items-center justify-center rounded-full border border-rec-strong/30 bg-rec-strong/10 p-0.5 transition-all duration-200 shrink-0"
          >
            <button
              type="button"
              class="flex size-7 items-center justify-center rounded-full border-none bg-rec-strong shadow-raised transition-all duration-150 enabled:cursor-pointer enabled:hover:scale-[1.05] enabled:active:scale-[0.95]"
              onclick={handleRecordToggle}
              title="Ferma registrazione"
              aria-label="Ferma registrazione"
            >
              <Square size={8} class="fill-current text-ink-primary" />
            </button>
          </div>
        {:else}
          <!-- Bezel idle circolare con play icon centrata poco a destra -->
          <div
            class="relative flex items-center justify-center rounded-full border border-brand-cream/10 bg-brand-darker/60 p-0.5 shadow-raised transition-all duration-200 shrink-0"
          >
            <button
              type="button"
              class={cn(
                "relative flex size-7 items-center justify-center rounded-full border-none bg-rec shadow-raised transition-all duration-150 enabled:cursor-pointer enabled:hover:bg-rec-strong enabled:hover:scale-[1.05] enabled:active:scale-[0.95]",
                !session.canRecord && "cursor-not-allowed opacity-50",
              )}
              disabled={!session.canRecord || session.isStarting}
              onclick={handleRecordToggle}
              title={session.canRecord
                ? "Avvia registrazione"
                : (session.recordGateReason ?? undefined)}
              aria-label="Avvia registrazione"
            >
              <Play
                size={11}
                fill="currentColor"
                strokeWidth={0}
                class="translate-x-0.5 pointer-events-none text-black/20"
              />
            </button>
          </div>
        {/if}

        <!-- Tasto Espandi (solo icona cliccabile, senza round button) -->
        <button
          type="button"
          class="flex size-7 items-center justify-center text-ink-faint hover:text-ink-primary active:scale-[0.92] transition-all cursor-pointer shrink-0"
          onclick={() => session.setCompactMode(false)}
          title="Espandi applicazione"
          aria-label="Espandi applicazione"
        >
          <Maximize2 size={13} />
        </button>

        <!-- I 6 puntini (GripVertical) posizionati a ridosso del bordo schermo destro -->
        <button
          type="button"
          class={cn(
            "flex h-7 w-4 items-center justify-center text-ink-faint hover:text-ink-primary cursor-grab active:cursor-grabbing transition-colors pr-0.5 shrink-0",
            isDragging && "text-rec cursor-grabbing",
          )}
          onpointerdown={handleGripPointerDown}
          title="Trascina in alto o in basso per spostare la linguetta"
          aria-label="Sposta linguetta"
        >
          <GripVertical size={13} />
        </button>
      </div>
    {/if}
  </div>
</div>
