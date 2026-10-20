//! Primitive condivise di processo/porta per i server locali (whisper, llm).
//! Estratte da `stt.rs` quando è arrivato un secondo server con lo stesso
//! ciclo di vita: la logica di stop porta due invarianti non ovvi (guard
//! rilasciato prima di ogni `.await`, mai kill-by-porta) che una copia-incolla
//! avrebbe finito per rompere in uno dei due punti.

use std::path::Path;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

// ── Lock del processo ─────────────────────────────────────────────────────────

/// Traccia su disco il processo di un server locale, per riconoscerlo dopo una
/// morte violenta dell'app (SIGKILL sul padre, Force Quit, logout), quando
/// `RunEvent::ExitRequested` non viene emesso e il figlio sopravvive tenendo la
/// porta. Senza, quel processo è indistinguibile da uno di terzi e l'app non
/// può né riusarlo né fermarlo.
#[derive(Serialize, Deserialize)]
pub(crate) struct ServerLock {
    pid: u32,
    bin: String,
    /// Istante di avvio del processo, nel formato grezzo di `ps -o lstart=`.
    /// È la difesa contro il riciclo dei pid: un pid da solo, in un lock vecchio,
    /// può puntare a un processo innocente.
    started_at: String,
    /// Modello caricato in RAM da quel processo. Un `llama-server` orfano può
    /// avere un GGUF diverso da quello ora selezionato: in quel caso va
    /// sostituito, non adottato.
    model: String,
}

/// `(bin, started_at)` del processo, letti in una sola chiamata. `lstart` è
/// primo perché occupa sempre 5 campi (`Mon Aug 24 10:35:22 2026`), così il
/// resto della riga è il path.
///
/// I campi si separano con `split_whitespace`, non con un singolo spazio: `ps`
/// padda il giorno del mese a due caratteri, quindi dall'1 al 9 la data arriva
/// con un doppio spazio (`Mon Aug  4 ...`) che sfaserebbe tutti i campi
/// successivi. Gli spazi vengono normalizzati a uno solo, sia qui sia quando il
/// lock viene scritto, così il confronto resta coerente.
fn process_identity(pid: u32) -> Option<(String, String)> {
    let out = std::process::Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "lstart=,comm="])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let line = String::from_utf8_lossy(&out.stdout);
    let mut parts = line.split_whitespace();
    let started_at: Vec<&str> = (&mut parts).take(5).collect();
    if started_at.len() < 5 {
        return None;
    }
    let bin = parts.collect::<Vec<_>>().join(" ");
    if bin.is_empty() {
        return None;
    }
    Some((bin, started_at.join(" ")))
}

/// Il processo del lock esiste ancora ed è davvero il nostro binario?
/// Il path deve combaciare per intero, o almeno nel nome del file: su Linux
/// `ps -o comm=` dà solo il nome base, non il path assoluto come su macOS.
/// L'istante di avvio invece deve combaciare esattamente, sempre.
fn lock_matches_process(lock: &ServerLock) -> bool {
    let Some((bin, started_at)) = process_identity(lock.pid) else {
        return false;
    };
    if started_at != lock.started_at {
        return false;
    }
    let same_file_name = |a: &str, b: &str| {
        Path::new(a).file_name().is_some() && Path::new(a).file_name() == Path::new(b).file_name()
    };
    bin == lock.bin || same_file_name(&bin, &lock.bin)
}

/// Lock del processo che occupa la porta, se e solo se quel processo è ancora
/// vivo ed è nostro. Un lock che non corrisponde più a nulla viene cancellato:
/// è spazzatura di una sessione passata, e lasciarlo lì significherebbe
/// riesaminarlo a ogni avvio.
pub(crate) fn read_valid_lock(lock_path: &Path) -> Option<ServerLock> {
    let lock: ServerLock = std::fs::read(lock_path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())?;
    if lock_matches_process(&lock) {
        return Some(lock);
    }
    let _ = std::fs::remove_file(lock_path);
    None
}

/// Il processo del lock sta girando con il modello che ci aspettiamo?
pub(crate) fn lock_has_model(lock: &ServerLock, model: &str) -> bool {
    lock.model == model
}

pub(crate) fn clear_lock(lock_path: &Path) {
    let _ = std::fs::remove_file(lock_path);
}

/// SIGKILL a un pid, per i processi che non abbiamo più come `Child` (un
/// orfano adottato). `kill(2)` è dichiarato a mano come gli altri extern del
/// progetto (`sysctlbyname` in `llm.rs`, `CGPreflightScreenCaptureAccess` in
/// `permissions.rs`): `libc` oggi è solo una dipendenza transitiva.
///
/// Da chiamare **solo** su un pid che `read_valid_lock` ha riconosciuto come
/// nostro: un pid non verificato può essere stato riciclato dall'OS su un
/// processo di qualcun altro.
pub(crate) fn kill_pid(pid: u32) {
    unsafe extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }
    const SIGKILL: i32 = 9;
    unsafe {
        kill(pid as i32, SIGKILL);
    }
}

pub(crate) async fn port_is_open(port: u16) -> bool {
    tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .is_ok()
}

#[allow(dead_code)]
pub(crate) async fn wait_for_port(port: u16, timeout_secs: u64) -> Result<(), String> {
    for _ in 0..timeout_secs * 4 {
        tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;
        if port_is_open(port).await {
            return Ok(());
        }
    }
    Err(format!("Server locale non pronto dopo {timeout_secs}s"))
}

/// Thread da passare ai server locali: tutti i core meno due (uno per l'app,
/// uno per l'OS), mai sotto 1.
pub(crate) fn default_threads() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get().saturating_sub(2).max(1))
        .unwrap_or(4)
}

/// Spawn di un server locale + registrazione del child nello slot tracciato,
/// così `stop_tracked_server` e il cleanup alla chiusura possono terminarlo.
///
/// `kill_on_drop(true)` è la rete di sicurezza per i percorsi in cui il `Child`
/// viene droppato senza un kill esplicito (unwind da panic, o un errore a metà
/// di `stop_tracked_server`): senza, il processo resterebbe vivo tenendo la
/// porta occupata e non tracciato da nessuno. Non copre la morte violenta
/// dell'app (SIGKILL sul padre): lì nessun codice del padre gira più.
pub(crate) fn spawn_tracked(
    bin: &Path,
    args: &[&str],
    slot: &Mutex<Option<tokio::process::Child>>,
    lock_path: &Path,
    model: &str,
) -> Result<(), String> {
    let child = tokio::process::Command::new(bin)
        .args(args)
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| e.to_string())?;

    // Il lock è best-effort: se non si riesce a scrivere, il server parte
    // comunque. Si perde solo la capacità di riconoscerlo come nostro dopo un
    // crash, che è esattamente la situazione di prima di questo meccanismo.
    if let Some(pid) = child.id() {
        if let Some((bin_path, started_at)) = process_identity(pid) {
            let lock = ServerLock {
                pid,
                bin: bin_path,
                started_at,
                model: model.to_string(),
            };
            if let Some(parent) = lock_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(json) = serde_json::to_vec(&lock) {
                let _ = std::fs::write(lock_path, json);
            }
        }
    }

    match slot.lock() {
        Ok(mut guard) => {
            *guard = Some(child);
            Ok(())
        }
        Err(e) => Err(format!("Stato del server corrotto: {e}")),
    }
}

/// Kill best-effort del child tracciato, usato alla chiusura dell'app: niente
/// `wait()` (il processo muore da solo, l'app sta uscendo).
///
/// Lo slot vuoto non significa "niente da fare": un server *adottato* (nato in
/// una sessione precedente, vedi `adopt_or_replace`) non è ricostruibile come
/// `Child`, quindi va terminato per pid. Senza questo ramo un orfano adottato
/// sopravvivrebbe a ogni chiusura e verrebbe riadottato all'infinito.
pub(crate) fn kill_tracked(slot: &Mutex<Option<tokio::process::Child>>, lock_path: &Path) {
    match slot.lock().ok().and_then(|mut guard| guard.take()) {
        Some(mut child) => {
            let _ = child.start_kill();
        }
        None => {
            if let Some(lock) = read_valid_lock(lock_path) {
                kill_pid(lock.pid);
            }
        }
    }
    clear_lock(lock_path);
}

/// Secondi di attesa dopo il kill prima di arrendersi: il processo può tenere
/// il socket ancora un istante dopo che `wait()` è tornato.
const PORT_RELEASE_TIMEOUT_SECS: u64 = 5;

/// Ferma il processo tracciato in `slot`, se presente. Non termina mai un
/// processo per porta: se lo slot è vuoto ma la porta è occupata, è un
/// processo che Earwig non ha avviato e va lasciato stare.
pub(crate) async fn stop_tracked_server(
    slot: &Mutex<Option<tokio::process::Child>>,
    port: u16,
    lock_path: &Path,
) -> Result<(), String> {
    // Il MutexGuard vive solo dentro il match: va tolto prima di qualsiasi
    // `.await` sul child (kill/wait), altrimenti il lock resterebbe tenuto
    // attraverso un punto di sospensione.
    let child = match slot.lock() {
        Ok(mut guard) => guard.take(),
        Err(e) => return Err(format!("Stato del server corrotto: {e}")),
    };

    match child {
        Some(mut child) => {
            // `take()` sopra ha già tolto il child dallo slot: se il segnale non
            // parte, il processo è ancora vivo e va rimesso dov'era, altrimenti
            // resterebbe senza nessuno che lo traccia (e `kill_tracked` alla
            // chiusura dell'app troverebbe lo slot vuoto). Un `wait` fallito
            // invece non si recupera: lì il SIGKILL è già partito.
            if let Err(e) = child.start_kill() {
                if let Ok(mut guard) = slot.lock() {
                    *guard = Some(child);
                }
                return Err(e.to_string());
            }
            child.wait().await.map_err(|e| e.to_string())?;
        }
        None => {
            if !port_is_open(port).await {
                clear_lock(lock_path);
                return Ok(());
            }
            // Slot vuoto e porta occupata: o è un server nostro adottato da una
            // sessione precedente (il lock lo dimostra, e allora si termina per
            // pid), o è un processo di terzi e non va toccato.
            match read_valid_lock(lock_path) {
                Some(lock) => kill_pid(lock.pid),
                None => {
                    return Err(format!(
                        "La porta {port} è occupata da un processo che Earwig non ha avviato: chiudilo manualmente prima di riprovare"
                    ))
                }
            }
        }
    }
    clear_lock(lock_path);
    wait_for_port_release(port).await
}

/// Attende che la porta torni libera dopo un kill: il processo può tenere il
/// socket ancora un istante dopo che il segnale è partito.
async fn wait_for_port_release(port: u16) -> Result<(), String> {
    if !port_is_open(port).await {
        return Ok(());
    }
    for _ in 0..PORT_RELEASE_TIMEOUT_SECS {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        if !port_is_open(port).await {
            return Ok(());
        }
    }

    Err(format!(
        "La porta {port} resta occupata {PORT_RELEASE_TIMEOUT_SECS}s dopo lo stop del server"
    ))
}

/// Esamina la porta prima di spawnare. Ritorna `true` quando c'è già un server
/// utilizzabile e lo spawn va saltato.
///
/// Il caso "processo non nostro" resta permissivo com'è sempre stato: l'app usa
/// quello che risponde sulla porta (è il flusso di chi avvia un server a mano
/// per debug, vedi la skill `run-earwig`), e sarà `stop_tracked_server` a
/// rifiutarsi di terminarlo. Il lock aggiunge solo i due casi che prima non si
/// potevano distinguere: un nostro orfano da riusare, e un nostro orfano che ha
/// in RAM il modello sbagliato e va sostituito.
pub(crate) async fn try_adopt_running_server(
    port: u16,
    lock_path: &Path,
    model: &str,
) -> Result<bool, String> {
    if !port_is_open(port).await {
        // Porta libera: un lock rimasto qui è di un processo ormai morto.
        clear_lock(lock_path);
        return Ok(false);
    }

    match read_valid_lock(lock_path) {
        Some(lock) if lock_has_model(&lock, model) => Ok(true),
        Some(lock) => {
            kill_pid(lock.pid);
            clear_lock(lock_path);
            wait_for_port_release(port).await?;
            Ok(false)
        }
        None => Ok(true),
    }
}
