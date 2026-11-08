//! OneDrive detection and account enumeration on Windows.

use crate::path::wide_to_path;
use explorer_domain::models::OneDriveItem;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use windows::core::{GUID, PCWSTR};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::System::Registry::{
    RegCloseKey, RegEnumKeyExW, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER,
    KEY_READ, REG_VALUE_TYPE,
};
use windows::Win32::UI::Shell::{KF_FLAG_DEFAULT, SHGetKnownFolderPath};

// FOLDERID_SkyDrive = {A52BBA46-E9E1-435F-B3D9-28DAA648C0F6}
pub const FOLDERID_SKYDRIVE: GUID = GUID::from_u128(0xA52BBA46_E9E1_435F_B3D9_28DAA648C0F6);

/// Reads a string value from an open Windows Registry key.
fn read_reg_string(hkey: HKEY, value_name: &str) -> Option<String> {
    let wide_name: Vec<u16> = value_name.encode_utf16().chain(Some(0)).collect();
    let mut reg_type = REG_VALUE_TYPE::default();
    let mut byte_size = 0u32;

    unsafe {
        let status = RegQueryValueExW(
            hkey,
            PCWSTR(wide_name.as_ptr()),
            None,
            Some(&mut reg_type),
            None,
            Some(&mut byte_size),
        );
        if status.is_err() || byte_size == 0 {
            return None;
        }

        let u16_len = (byte_size as usize) / 2;
        let mut buffer = vec![0u16; u16_len + 1];
        let status = RegQueryValueExW(
            hkey,
            PCWSTR(wide_name.as_ptr()),
            None,
            Some(&mut reg_type),
            Some(buffer.as_mut_ptr() as *mut u8),
            Some(&mut byte_size),
        );
        if status.is_err() {
            return None;
        }

        let actual_len = buffer.iter().position(|&c| c == 0).unwrap_or(u16_len);
        let val = String::from_utf16_lossy(&buffer[..actual_len]).trim().to_string();
        if val.is_empty() {
            None
        } else {
            Some(val)
        }
    }
}

/// Normalizes a path string for deduplication (case-insensitive on Windows, trimmed).
fn normalize_path_key(path: &str) -> String {
    path.trim_end_matches(['\\', '/']).to_lowercase()
}

/// Retrieves the filesystem path for FOLDERID_SkyDrive via Shell API.
pub fn get_known_skydrive_path() -> Option<PathBuf> {
    unsafe {
        match SHGetKnownFolderPath(&FOLDERID_SKYDRIVE, KF_FLAG_DEFAULT, HANDLE::default()) {
            Ok(path_pwstr) if !path_pwstr.is_null() => {
                let slice = path_pwstr.as_wide();
                let path = wide_to_path(slice);
                CoTaskMemFree(Some(path_pwstr.as_ptr() as *const _));
                if path.is_dir() {
                    Some(path)
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

/// Scans the Windows Registry `HKCU\Software\Microsoft\OneDrive\Accounts` for configured accounts.
fn detect_registry_accounts() -> Vec<OneDriveItem> {
    let mut items = Vec::new();
    let accounts_path: Vec<u16> = "Software\\Microsoft\\OneDrive\\Accounts\0"
        .encode_utf16()
        .collect();

    let mut h_accounts = HKEY::default();
    let res = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(accounts_path.as_ptr()),
            0,
            KEY_READ,
            &mut h_accounts,
        )
    };
    if res.is_err() {
        return items;
    }

    let mut index = 0u32;
    let mut name_buf = [0u16; 256];
    loop {
        let mut name_len = name_buf.len() as u32;
        let enum_res = unsafe {
            RegEnumKeyExW(
                h_accounts,
                index,
                windows::core::PWSTR(name_buf.as_mut_ptr()),
                &mut name_len,
                None,
                windows::core::PWSTR::null(),
                None,
                None,
            )
        };
        if enum_res.is_err() {
            break;
        }

        let subkey_name = String::from_utf16_lossy(&name_buf[..name_len as usize]);
        index += 1;

        let subkey_wide: Vec<u16> = subkey_name.encode_utf16().chain(Some(0)).collect();
        let mut h_account = HKEY::default();
        let open_sub = unsafe {
            RegOpenKeyExW(
                h_accounts,
                PCWSTR(subkey_wide.as_ptr()),
                0,
                KEY_READ,
                &mut h_account,
            )
        };

        if open_sub.is_ok() {
            if let Some(user_folder) = read_reg_string(h_account, "UserFolder") {
                if Path::new(&user_folder).is_dir() {
                    let display_name = read_reg_string(h_account, "DisplayName");
                    let user_email = read_reg_string(h_account, "UserEmail");
                    let account_type = if subkey_name.eq_ignore_ascii_case("Personal") {
                        "Personal".to_string()
                    } else if subkey_name.to_lowercase().starts_with("business") {
                        "Business".to_string()
                    } else {
                        "OneDrive".to_string()
                    };

                    let label = if let Some(dn) = display_name {
                        dn
                    } else if account_type == "Business" {
                        "OneDrive - Business".to_string()
                    } else {
                        "OneDrive".to_string()
                    };

                    items.push(OneDriveItem {
                        id: format!("onedrive_{}", subkey_name.to_lowercase()),
                        name: label,
                        path: user_folder,
                        account_type,
                        email: user_email,
                    });
                }
            }
            unsafe {
                let _ = RegCloseKey(h_account);
            }
        }
    }

    unsafe {
        let _ = RegCloseKey(h_accounts);
    }

    items
}

/// Fallback check in `HKCU\Software\Microsoft\OneDrive` root key.
fn detect_root_registry_account() -> Option<OneDriveItem> {
    let onedrive_path: Vec<u16> = "Software\\Microsoft\\OneDrive\0".encode_utf16().collect();
    let mut h_onedrive = HKEY::default();
    let res = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(onedrive_path.as_ptr()),
            0,
            KEY_READ,
            &mut h_onedrive,
        )
    };
    if res.is_err() {
        return None;
    }

    let user_folder = read_reg_string(h_onedrive, "UserFolder");
    unsafe {
        let _ = RegCloseKey(h_onedrive);
    }

    if let Some(folder) = user_folder {
        if Path::new(&folder).is_dir() {
            return Some(OneDriveItem {
                id: "onedrive_root".to_string(),
                name: "OneDrive".to_string(),
                path: folder,
                account_type: "Personal".to_string(),
                email: None,
            });
        }
    }
    None
}

/// Detects all logged-in OneDrive accounts on the current Windows machine.
/// Returns an empty list if the user is not logged in to OneDrive or OneDrive folders do not exist.
pub fn get_onedrive_accounts() -> Vec<OneDriveItem> {
    let mut accounts = Vec::new();
    let mut seen_paths = HashSet::new();

    // 1. Primary: HKCU\Software\Microsoft\OneDrive\Accounts subkeys
    for item in detect_registry_accounts() {
        let key = normalize_path_key(&item.path);
        if seen_paths.insert(key) {
            accounts.push(item);
        }
    }

    // 2. Fallback: HKCU\Software\Microsoft\OneDrive\UserFolder
    if let Some(item) = detect_root_registry_account() {
        let key = normalize_path_key(&item.path);
        if seen_paths.insert(key) {
            accounts.push(item);
        }
    }

    // 3. Fallback: Shell Known Folder FOLDERID_SkyDrive
    if let Some(skydrive_path) = get_known_skydrive_path() {
        let path_str = skydrive_path.to_string_lossy().to_string();
        let key = normalize_path_key(&path_str);
        if seen_paths.insert(key) {
            accounts.push(OneDriveItem {
                id: "onedrive_skydrive".to_string(),
                name: "OneDrive".to_string(),
                path: path_str,
                account_type: "Personal".to_string(),
                email: None,
            });
        }
    }

    // 4. Fallback: Environment variables (OneDriveConsumer, OneDriveCommercial, OneDrive)
    let env_candidates = [
        ("OneDriveConsumer", "Personal"),
        ("OneDriveCommercial", "Business"),
        ("OneDrive", "Personal"),
    ];
    for (env_var, acc_type) in env_candidates {
        if let Ok(val) = std::env::var(env_var) {
            let p = Path::new(&val);
            if p.is_dir() {
                let path_str = p.to_string_lossy().to_string();
                let key = normalize_path_key(&path_str);
                if seen_paths.insert(key) {
                    let label = if acc_type == "Business" {
                        "OneDrive - Business"
                    } else {
                        "OneDrive"
                    };
                    accounts.push(OneDriveItem {
                        id: format!("onedrive_env_{}", env_var.to_lowercase()),
                        name: label.to_string(),
                        path: path_str,
                        account_type: acc_type.to_string(),
                        email: None,
                    });
                }
            }
        }
    }

    // If multiple accounts exist (e.g. Personal + Business), disambiguate names if both are just "OneDrive"
    if accounts.len() > 1 {
        for acc in &mut accounts {
            if acc.name == "OneDrive" {
                acc.name = format!("OneDrive - {}", acc.account_type);
            }
        }
    }

    accounts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_path_key() {
        assert_eq!(
            normalize_path_key(r"C:\Users\User\OneDrive\"),
            normalize_path_key(r"c:\users\user\onedrive")
        );
    }

    #[test]
    fn test_onedrive_detection_on_current_host() {
        let accounts = get_onedrive_accounts();
        println!("Detected OneDrive accounts: {:?}", accounts);
        // If host has OneDrive logged in, verify attributes
        if !accounts.is_empty() {
            let first = &accounts[0];
            assert!(first.path.to_lowercase().contains("onedrive"));
            assert!(Path::new(&first.path).is_dir());
        }
    }

    #[test]
    fn test_get_onedrive_accounts_no_panic() {
        let accounts = get_onedrive_accounts();
        // Should execute smoothly without crashing on any Windows host
        for acc in &accounts {
            assert!(!acc.id.is_empty());
            assert!(!acc.name.is_empty());
            assert!(!acc.path.is_empty());
            assert!(Path::new(&acc.path).exists());
        }
    }

    #[test]
    fn test_known_skydrive_path_no_panic() {
        let _ = get_known_skydrive_path();
    }
}
