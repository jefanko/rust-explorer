#![allow(dead_code, unsafe_op_in_unsafe_fn)]

use crate::resources::SafeBrush;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use windows::Win32::Foundation::{
    BOOL, COLORREF, ERROR_SUCCESS, FARPROC, HMODULE, HWND, LPARAM, WPARAM,
};
use windows::Win32::Graphics::Dwm::{
    DWMWA_BORDER_COLOR, DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{
    COLOR_BTNFACE, COLOR_GRAYTEXT, COLOR_WINDOW, COLOR_WINDOWTEXT, CreateSolidBrush, GetSysColor,
    GetSysColorBrush, HBRUSH, InvalidateRect,
};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
use windows::Win32::UI::Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW};
use windows::Win32::UI::Controls::SetWindowTheme;
use windows::Win32::UI::WindowsAndMessaging::{
    EnumChildWindows, SPI_GETHIGHCONTRAST, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SendMessageW,
    SystemParametersInfoW, WM_THEMECHANGED,
};
use windows::core::{PCSTR, PCWSTR, w};

/// Window background and control gaps.
pub const DARK_BG: u32 = 0x0019_1919;
/// Toolbar rows, status bar, sidebar and preview container.
pub const DARK_PANEL: u32 = 0x0020_2020;
/// Dialog and popup-like surfaces.
pub const DARK_SURFACE: u32 = 0x002C_2C2C;
/// Address, search and preview text fields.
pub const DARK_INPUT: u32 = 0x002B_2B2B;
/// Primary text.
pub const DARK_TEXT: u32 = 0x00FF_FFFF;
/// Status bar and secondary text.
pub const DARK_TEXT_SECONDARY: u32 = 0x00A6_A6A6;
/// Disabled and cue text.
pub const DARK_TEXT_MUTED: u32 = 0x006E_6E6E;
/// Borders and list dividers.
pub const DARK_BORDER: u32 = 0x002D_2D2D;
/// Accent color, kept for selection and focus work.
pub const DARK_ACCENT: u32 = 0x00FF_CD60;

const DWMWA_COLOR_DEFAULT: u32 = 0xFFFF_FFFF;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemePreference {
    System,
    Light,
    Dark,
}

impl ThemePreference {
    pub fn from_setting(value: &str) -> Self {
        match value.to_ascii_lowercase().as_str() {
            "light" => Self::Light,
            "dark" => Self::Dark,
            _ => Self::System,
        }
    }

    pub fn as_setting(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }

    pub fn cycle(self) -> Self {
        match self {
            Self::System => Self::Light,
            Self::Light => Self::Dark,
            Self::Dark => Self::System,
        }
    }

    pub fn resolve(self, os_dark: bool, high_contrast: bool) -> bool {
        if high_contrast {
            return false;
        }
        match self {
            Self::System => os_dark,
            Self::Light => false,
            Self::Dark => true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeControl {
    Button,
    Tree,
    List,
    Header,
    Edit,
    Tab,
    Status,
}

pub struct ThemeState {
    pub preference: ThemePreference,
    pub dark: bool,
    pub high_contrast: bool,
    background: SafeBrush,
    panel: SafeBrush,
    surface: SafeBrush,
    input: SafeBrush,
    accent: SafeBrush,
}

impl ThemeState {
    pub fn new(preference: ThemePreference) -> Self {
        let high_contrast = high_contrast_enabled();
        let dark = preference.resolve(os_prefers_dark(), high_contrast);
        let state = Self {
            preference,
            dark,
            high_contrast,
            background: solid_brush(DARK_BG),
            panel: solid_brush(DARK_PANEL),
            surface: solid_brush(DARK_SURFACE),
            input: solid_brush(DARK_INPUT),
            accent: solid_brush(DARK_ACCENT),
        };
        if !high_contrast {
            set_preferred_app_mode(state.app_mode());
            flush_menu_themes();
        }
        state
    }

    /// Process-wide uxtheme mode: explicit Dark overrides a light OS, while
    /// System only allows dark mode when the OS is dark.
    pub fn app_mode(&self) -> i32 {
        if self.dark {
            if self.preference == ThemePreference::Dark {
                2
            } else {
                1
            }
        } else {
            3
        }
    }

    /// Re-reads the OS theme and accessibility settings. Returns true when the
    /// resolved appearance changed.
    pub fn refresh(&mut self) -> bool {
        let high_contrast = high_contrast_enabled();
        let dark = self.preference.resolve(os_prefers_dark(), high_contrast);
        let changed = dark != self.dark || high_contrast != self.high_contrast;
        self.dark = dark;
        self.high_contrast = high_contrast;
        changed
    }

    pub fn background_brush(&self) -> HBRUSH {
        self.background.handle()
    }

    pub fn panel_brush(&self) -> HBRUSH {
        self.panel.handle()
    }

    pub fn surface_brush(&self) -> HBRUSH {
        self.surface.handle()
    }

    pub fn input_brush(&self) -> HBRUSH {
        self.input.handle()
    }

    pub fn accent_brush(&self) -> HBRUSH {
        self.accent.handle()
    }
}

pub struct DialogPalette {
    pub dark: bool,
    surface: Option<SafeBrush>,
    input: Option<SafeBrush>,
    surface_color: COLORREF,
    input_color: COLORREF,
}

impl DialogPalette {
    pub fn new(dark: bool) -> Self {
        if dark {
            Self {
                dark,
                surface: Some(solid_brush(DARK_SURFACE)),
                input: Some(solid_brush(DARK_INPUT)),
                surface_color: COLORREF(DARK_SURFACE),
                input_color: COLORREF(DARK_INPUT),
            }
        } else {
            Self {
                dark,
                surface: None,
                input: None,
                surface_color: COLORREF(unsafe { GetSysColor(COLOR_BTNFACE) }),
                input_color: COLORREF(unsafe { GetSysColor(COLOR_WINDOW) }),
            }
        }
    }

    pub fn background_brush(&self) -> HBRUSH {
        match self.surface.as_ref() {
            Some(brush) => brush.handle(),
            None => unsafe { GetSysColorBrush(COLOR_BTNFACE) },
        }
    }

    pub fn input_brush(&self) -> HBRUSH {
        match self.input.as_ref() {
            Some(brush) => brush.handle(),
            None => unsafe { GetSysColorBrush(COLOR_WINDOW) },
        }
    }

    pub fn background_color(&self) -> COLORREF {
        self.surface_color
    }

    pub fn input_color(&self) -> COLORREF {
        self.input_color
    }

    pub fn text_color(&self) -> COLORREF {
        if self.dark {
            COLORREF(DARK_TEXT)
        } else {
            COLORREF(unsafe { GetSysColor(COLOR_WINDOWTEXT) })
        }
    }

    pub fn secondary_color(&self) -> COLORREF {
        if self.dark {
            COLORREF(DARK_TEXT_SECONDARY)
        } else {
            COLORREF(unsafe { GetSysColor(COLOR_GRAYTEXT) })
        }
    }
}

fn solid_brush(color: u32) -> SafeBrush {
    unsafe { SafeBrush::new(CreateSolidBrush(COLORREF(color))) }
}

static ACTIVE_DARK: AtomicBool = AtomicBool::new(false);
static ACTIVE_HIGH_CONTRAST: AtomicBool = AtomicBool::new(false);

pub fn set_active(dark: bool, high_contrast: bool) {
    ACTIVE_DARK.store(dark, Ordering::Relaxed);
    ACTIVE_HIGH_CONTRAST.store(high_contrast, Ordering::Relaxed);
}

pub fn active_dark() -> bool {
    ACTIVE_DARK.load(Ordering::Relaxed)
}

pub fn active_high_contrast() -> bool {
    ACTIVE_HIGH_CONTRAST.load(Ordering::Relaxed)
}

type SetPreferredAppModeFn = unsafe extern "system" fn(i32) -> i32;
type AllowDarkModeForWindowFn = unsafe extern "system" fn(HWND, BOOL) -> BOOL;
type FlushMenuThemesFn = unsafe extern "system" fn();
type RefreshImmersiveColorPolicyStateFn = unsafe extern "system" fn();
type ShouldAppsUseDarkModeFn = unsafe extern "system" fn() -> u8;

struct UxTheme {
    set_preferred_app_mode: Option<SetPreferredAppModeFn>,
    allow_dark_mode_for_window: Option<AllowDarkModeForWindowFn>,
    flush_menu_themes: Option<FlushMenuThemesFn>,
    refresh_immersive_color_policy_state: Option<RefreshImmersiveColorPolicyStateFn>,
    should_apps_use_dark_mode: Option<ShouldAppsUseDarkModeFn>,
}

static UXTHEME: OnceLock<UxTheme> = OnceLock::new();

fn uxtheme() -> &'static UxTheme {
    UXTHEME.get_or_init(|| unsafe {
        let Ok(module) = LoadLibraryW(w!("uxtheme.dll")) else {
            return UxTheme {
                set_preferred_app_mode: None,
                allow_dark_mode_for_window: None,
                flush_menu_themes: None,
                refresh_immersive_color_policy_state: None,
                should_apps_use_dark_mode: None,
            };
        };
        UxTheme {
            set_preferred_app_mode: load_ordinal(module, 135),
            allow_dark_mode_for_window: load_ordinal(module, 133),
            flush_menu_themes: load_ordinal(module, 136),
            refresh_immersive_color_policy_state: load_ordinal(module, 104),
            should_apps_use_dark_mode: load_ordinal(module, 132),
        }
    })
}

unsafe fn load_ordinal<T>(module: HMODULE, ordinal: usize) -> Option<T> {
    let proc: FARPROC = GetProcAddress(module, PCSTR(ordinal as *const u8));
    proc.map(|function| unsafe { std::mem::transmute_copy(&function) })
}

pub fn set_preferred_app_mode(mode: i32) {
    if let Some(function) = uxtheme().set_preferred_app_mode {
        unsafe {
            let _ = function(mode);
        }
    }
    if let Some(function) = uxtheme().refresh_immersive_color_policy_state {
        unsafe {
            function();
        }
    }
}

pub fn flush_menu_themes() {
    if let Some(function) = uxtheme().flush_menu_themes {
        unsafe {
            function();
        }
    }
}

pub fn allow_dark_mode_for_window(hwnd: HWND, dark: bool) {
    if let Some(function) = uxtheme().allow_dark_mode_for_window {
        unsafe {
            let _ = function(hwnd, BOOL::from(dark));
        }
    }
}

pub fn os_prefers_dark() -> bool {
    if let Some(function) = uxtheme().should_apps_use_dark_mode {
        return unsafe { function() != 0 };
    }
    registry_apps_use_dark().unwrap_or(false)
}

fn registry_apps_use_dark() -> Option<bool> {
    let mut value: u32 = 1;
    let mut size = std::mem::size_of::<u32>() as u32;
    let result = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("AppsUseLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut value as *mut u32 as *mut core::ffi::c_void),
            Some(&mut size),
        )
    };
    (result == ERROR_SUCCESS).then_some(value == 0)
}

pub fn high_contrast_enabled() -> bool {
    let mut settings = HIGHCONTRASTW {
        cbSize: std::mem::size_of::<HIGHCONTRASTW>() as u32,
        ..Default::default()
    };
    let ok = unsafe {
        SystemParametersInfoW(
            SPI_GETHIGHCONTRAST,
            settings.cbSize,
            Some(&mut settings as *mut HIGHCONTRASTW as *mut core::ffi::c_void),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
        .is_ok()
    };
    ok && (settings.dwFlags.0 & HCF_HIGHCONTRASTON.0) != 0
}

pub fn apply_window_chrome(hwnd: HWND, dark: bool) {
    if hwnd.is_invalid() {
        return;
    }
    unsafe {
        let enabled = BOOL::from(dark);
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &enabled as *const BOOL as *const core::ffi::c_void,
            std::mem::size_of::<BOOL>() as u32,
        );
        let border = COLORREF(if dark {
            DARK_BORDER
        } else {
            DWMWA_COLOR_DEFAULT
        });
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR,
            &border as *const COLORREF as *const core::ffi::c_void,
            std::mem::size_of::<COLORREF>() as u32,
        );
    }
}

pub fn apply_control_theme(hwnd: HWND, dark: bool, control: ThemeControl) {
    if hwnd.is_invalid() {
        return;
    }
    let name = match (dark, control) {
        (true, ThemeControl::List) => w!("DarkMode_Explorer"),
        (false, ThemeControl::List) => w!("Explorer"),
        (true, ThemeControl::Header) => w!("DarkMode_ItemsView"),
        (false, ThemeControl::Header) => w!("ItemsView"),
        (true, ThemeControl::Edit) => w!("DarkMode_CFD"),
        (false, ThemeControl::Edit) => w!("Explorer"),
        (true, ThemeControl::Tab | ThemeControl::Status) => w!(""),
        (false, ThemeControl::Tab | ThemeControl::Status) => w!("Explorer"),
        (_, ThemeControl::Button | ThemeControl::Tree) => w!("Explorer"),
    };
    let _ = unsafe { SetWindowTheme(hwnd, name, PCWSTR::null()) };
    allow_dark_mode_for_window(hwnd, dark);
    unsafe {
        let _ = SendMessageW(hwnd, WM_THEMECHANGED, WPARAM(0), LPARAM(0));
        let _ = InvalidateRect(hwnd, None, true);
    }
}

pub fn apply_dialog_theme(hwnd: HWND, dark: bool) {
    if hwnd.is_invalid() {
        return;
    }
    apply_window_chrome(hwnd, dark);
    apply_control_theme(hwnd, dark, ThemeControl::Button);
    unsafe {
        let _ = EnumChildWindows(hwnd, Some(theme_child_window), LPARAM(dark as isize));
        let _ = InvalidateRect(hwnd, None, true);
    }
}

unsafe extern "system" fn theme_child_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let dark = lparam.0 != 0;
    let mut buffer = [0u16; 64];
    let length = windows::Win32::UI::WindowsAndMessaging::GetClassNameW(hwnd, &mut buffer);
    let class = String::from_utf16_lossy(&buffer[..length.max(0) as usize]);
    let control = match class.as_str() {
        "Edit" => ThemeControl::Edit,
        _ => ThemeControl::Button,
    };
    apply_control_theme(hwnd, dark, control);
    BOOL::from(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preference_parses_and_round_trips() {
        assert_eq!(
            ThemePreference::from_setting("system"),
            ThemePreference::System
        );
        assert_eq!(
            ThemePreference::from_setting("LIGHT"),
            ThemePreference::Light
        );
        assert_eq!(ThemePreference::from_setting("dark"), ThemePreference::Dark);
        assert_eq!(
            ThemePreference::from_setting("nonsense"),
            ThemePreference::System
        );
        for preference in [
            ThemePreference::System,
            ThemePreference::Light,
            ThemePreference::Dark,
        ] {
            assert_eq!(
                ThemePreference::from_setting(preference.as_setting()),
                preference
            );
        }
    }

    #[test]
    fn preference_cycles_system_light_dark() {
        assert_eq!(ThemePreference::System.cycle(), ThemePreference::Light);
        assert_eq!(ThemePreference::Light.cycle(), ThemePreference::Dark);
        assert_eq!(ThemePreference::Dark.cycle(), ThemePreference::System);
    }

    #[test]
    fn resolve_respects_preference_and_system_theme() {
        assert!(ThemePreference::System.resolve(true, false));
        assert!(!ThemePreference::System.resolve(false, false));
        assert!(ThemePreference::Dark.resolve(false, false));
        assert!(!ThemePreference::Light.resolve(true, false));
    }

    #[test]
    fn high_contrast_disables_the_custom_dark_palette() {
        assert!(!ThemePreference::Dark.resolve(true, true));
        assert!(!ThemePreference::System.resolve(true, true));
    }
}
