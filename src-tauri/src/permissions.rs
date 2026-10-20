//! Stato e gestione dei permessi OS richiesti da Earwig (microfono, registrazione
//! schermo). Vive fuori da `recorder/`: non è cattura audio, è integrazione OS.

#[cfg(target_os = "macos")]
mod macos {
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGPreflightScreenCaptureAccess() -> bool;
    }

    #[link(name = "AVFoundation", kind = "framework")]
    #[link(name = "objc", kind = "dylib")]
    unsafe extern "C" {
        fn objc_getClass(name: *const std::os::raw::c_char) -> *mut std::ffi::c_void;
        fn sel_registerName(name: *const std::os::raw::c_char) -> *mut std::ffi::c_void;
        fn objc_msgSend();
        static AVMediaTypeAudio: *mut std::ffi::c_void;
    }

    pub fn screen_recording_granted() -> bool {
        unsafe { CGPreflightScreenCaptureAccess() }
    }

    pub fn microphone_granted() -> bool {
        unsafe {
            let cls = objc_getClass(b"AVCaptureDevice\0".as_ptr() as *const _);
            if cls.is_null() {
                return false;
            }
            let sel = sel_registerName(b"authorizationStatusForMediaType:\0".as_ptr() as *const _);
            if sel.is_null() {
                return false;
            }
            let msg_send: unsafe extern "C" fn(
                *mut std::ffi::c_void,
                *mut std::ffi::c_void,
                *mut std::ffi::c_void,
            ) -> isize = std::mem::transmute(objc_msgSend as *const ());
            let status = msg_send(cls, sel, AVMediaTypeAudio);
            // AVAuthorizationStatusNotDetermined = 0
            // AVAuthorizationStatusRestricted = 1
            // AVAuthorizationStatusDenied = 2
            // AVAuthorizationStatusAuthorized = 3
            status == 3
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod macos {
    pub fn screen_recording_granted() -> bool {
        true
    }

    pub fn microphone_granted() -> bool {
        true
    }
}

pub fn screen_recording_granted() -> bool {
    macos::screen_recording_granted()
}

pub fn microphone_granted() -> bool {
    macos::microphone_granted()
}

pub enum PermissionPane {
    Microphone,
    ScreenRecording,
}

/// Apre il pannello di Privacy & Security pertinente in System Settings.
/// `pane` è un enum chiuso: nessuna stringa esterna finisce nel comando.
pub fn open_settings_pane(pane: PermissionPane) -> Result<(), String> {
    let url = match pane {
        PermissionPane::Microphone => {
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone"
        }
        PermissionPane::ScreenRecording => {
            "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture"
        }
    };

    std::process::Command::new("open")
        .arg(url)
        .spawn()
        .map_err(|e| format!("Impossibile aprire Impostazioni di Sistema: {e}"))?;

    Ok(())
}
