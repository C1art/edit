//! Opening projects from Explorer: set up when Cledit is installed or updated, removed
//! when it is uninstalled. Per user, so no administrator rights are needed.

#[cfg(windows)]
pub(super) use windows_registry::{register, unregister};

#[cfg(windows)]
mod windows_registry {
    use windows::core::{HSTRING, PCWSTR};
    use windows::Win32::System::Registry::{
        RegCloseKey, RegCreateKeyExW, RegDeleteTreeW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
        KEY_WRITE, REG_OPTION_NON_VOLATILE, REG_SZ,
    };
    use windows::Win32::UI::Shell::{SHChangeNotify, SHCNE_ASSOCCHANGED, SHCNF_IDLIST};

    const PROG_ID: &str = "Cledit.Project";
    const CLASSES: &str = r"Software\Classes";

    pub(crate) fn register() {
        let Ok(exe) = std::env::current_exe() else {
            return;
        };
        let exe = exe.display();
        set_default(&format!(r"{CLASSES}\{PROG_ID}"), "Cledit Project");
        set_default(
            &format!(r"{CLASSES}\{PROG_ID}\DefaultIcon"),
            &format!("\"{exe}\",0"),
        );
        set_default(
            &format!(r"{CLASSES}\{PROG_ID}\shell\open\command"),
            &format!("\"{exe}\" \"%1\""),
        );
        for extension in [project::EXTENSION, project::OLD_EXTENSION] {
            set_default(&format!(r"{CLASSES}\.{extension}"), PROG_ID);
        }
        icons_changed();
    }

    pub(crate) fn unregister() {
        let keys = [project::EXTENSION, project::OLD_EXTENSION]
            .map(|extension| format!(r"{CLASSES}\.{extension}"));
        for key in keys.iter().chain([&format!(r"{CLASSES}\{PROG_ID}")]) {
            // SAFETY: a predefined root key and a string that outlives the call.
            let _ = unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, &HSTRING::from(key.as_str())) };
        }
        icons_changed();
    }

    /// Sets a key's unnamed value, creating the key if needed.
    fn set_default(key: &str, value: &str) {
        let mut opened = HKEY::default();
        // SAFETY: a predefined root key, strings that outlive the call, and a handle
        // written only on success and closed below.
        let created = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                &HSTRING::from(key),
                None,
                PCWSTR::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_WRITE,
                None,
                &mut opened,
                None,
            )
        };
        if created.is_err() {
            return;
        }
        let bytes: Vec<u8> = value
            .encode_utf16()
            .chain([0])
            .flat_map(u16::to_le_bytes)
            .collect();
        // SAFETY: the key opened above, and REG_SZ data ending in a null character.
        let _ = unsafe { RegSetValueExW(opened, PCWSTR::null(), None, REG_SZ, Some(&bytes)) };
        // SAFETY: closes the handle opened above, once.
        let _ = unsafe { RegCloseKey(opened) };
    }

    /// Explorer redraws project files with the new icon and opener.
    fn icons_changed() {
        // SAFETY: SHCNE_ASSOCCHANGED takes no items.
        unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None) };
    }
}
