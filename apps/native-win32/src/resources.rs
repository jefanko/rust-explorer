#![allow(dead_code)]

use windows::Win32::Graphics::Gdi::{DeleteObject, HBITMAP, HBRUSH, HFONT, HGDIOBJ};
use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, HICON};

/// RAII wrapper for an owned HFONT handle.
pub struct SafeFont(HFONT);

impl SafeFont {
    pub fn new(font: HFONT) -> Self {
        Self(font)
    }

    pub fn handle(&self) -> HFONT {
        self.0
    }
}

impl Drop for SafeFont {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            unsafe {
                let _ = DeleteObject(HGDIOBJ(self.0.0));
            }
        }
    }
}

/// RAII wrapper for an owned HBRUSH handle.
pub struct SafeBrush(HBRUSH);

impl SafeBrush {
    pub fn new(brush: HBRUSH) -> Self {
        Self(brush)
    }

    pub fn handle(&self) -> HBRUSH {
        self.0
    }
}

impl Drop for SafeBrush {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            unsafe {
                let _ = DeleteObject(HGDIOBJ(self.0.0));
            }
        }
    }
}

/// RAII wrapper for an owned HBITMAP handle.
pub struct SafeBitmap(HBITMAP);

impl SafeBitmap {
    pub fn new(bitmap: HBITMAP) -> Self {
        Self(bitmap)
    }

    pub fn handle(&self) -> HBITMAP {
        self.0
    }
}

impl Drop for SafeBitmap {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            unsafe {
                let _ = DeleteObject(HGDIOBJ(self.0.0));
            }
        }
    }
}

/// RAII wrapper for an owned HICON handle.
pub struct SafeIcon(HICON);

impl SafeIcon {
    pub fn new(icon: HICON) -> Self {
        Self(icon)
    }

    pub fn handle(&self) -> HICON {
        self.0
    }
}

impl Drop for SafeIcon {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            unsafe {
                let _ = DestroyIcon(self.0);
            }
        }
    }
}
