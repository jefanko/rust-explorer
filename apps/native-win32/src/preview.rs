#![allow(dead_code, unsafe_op_in_unsafe_fn)]

use crate::resources::SafeBitmap;
use crate::theme::{self, ThemeControl};
use explorer_domain::models::PreviewData;
use windows::Win32::Foundation::{HINSTANCE, HWND};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, w};

pub struct PreviewPane {
    pub hwnd_container: HWND,
    pub hwnd_text: HWND,
    pub hwnd_header: HWND,
    pub current_image: Option<SafeBitmap>,
    pub current_image_size: (u32, u32),
    pub visible: bool,
}

impl PreviewPane {
    pub fn new(hwnd_parent: HWND, hinstance: HINSTANCE) -> Self {
        let hwnd_container = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("STATIC"),
                PCWSTR::null(),
                WS_CHILD | WS_CLIPCHILDREN,
                0,
                0,
                0,
                0,
                hwnd_parent,
                None,
                hinstance,
                None,
            )
            .unwrap()
        };

        let hwnd_header = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("STATIC"),
                w!("No item selected"),
                WS_CHILD | WS_VISIBLE,
                10,
                10,
                260,
                80,
                hwnd_container,
                None,
                hinstance,
                None,
            )
            .unwrap()
        };

        let hwnd_text = unsafe {
            CreateWindowExW(
                WS_EX_CLIENTEDGE,
                w!("EDIT"),
                PCWSTR::null(),
                WS_CHILD
                    | WS_VISIBLE
                    | WS_VSCROLL
                    | WS_HSCROLL
                    | WINDOW_STYLE(
                        ES_MULTILINE as u32
                            | ES_READONLY as u32
                            | ES_AUTOVSCROLL as u32
                            | ES_AUTOHSCROLL as u32,
                    ),
                10,
                100,
                260,
                300,
                hwnd_container,
                None,
                hinstance,
                None,
            )
            .unwrap()
        };

        Self {
            hwnd_container,
            hwnd_text,
            hwnd_header,
            current_image: None,
            current_image_size: (0, 0),
            visible: false,
        }
    }

    /// Applies the host UI font to the header and the text surface.
    pub fn apply_font(&self, font: windows::Win32::Graphics::Gdi::HFONT) {
        unsafe {
            for control in [self.hwnd_header, self.hwnd_text] {
                let _ = SendMessageW(
                    control,
                    WM_SETFONT,
                    windows::Win32::Foundation::WPARAM(font.0 as usize),
                    windows::Win32::Foundation::LPARAM(1),
                );
            }
        }
    }

    pub fn set_bounds(&self, x: i32, y: i32, w: i32, h: i32) {
        unsafe {
            let _ = MoveWindow(self.hwnd_container, x, y, w, h, true);
            let header_h = 75;
            let pad = 10;
            let inner_w = (w - pad * 2).max(10);
            let _ = MoveWindow(self.hwnd_header, pad, pad, inner_w, header_h, true);
            let text_y = header_h + pad * 2;
            let text_h = (h - text_y - pad).max(10);
            let _ = MoveWindow(self.hwnd_text, pad, text_y, inner_w, text_h, true);
        }
    }

    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
        unsafe {
            let _ = ShowWindow(self.hwnd_container, if visible { SW_SHOW } else { SW_HIDE });
        }
    }

    pub fn apply_theme(&self, dark: bool) {
        theme::apply_control_theme(self.hwnd_container, dark, ThemeControl::Button);
        theme::apply_control_theme(self.hwnd_header, dark, ThemeControl::Button);
        theme::apply_control_theme(self.hwnd_text, dark, ThemeControl::Edit);
    }

    pub fn show_preview(&mut self, data: &PreviewData) {
        self.current_image = None;
        match data {
            PreviewData::Text {
                content,
                truncated,
                encoding,
                line_count,
            } => {
                let header = format!(
                    "Text Document\nEncoding: {}\nLines: {}{}",
                    encoding,
                    line_count,
                    if *truncated {
                        " (truncated at 64 KB)"
                    } else {
                        ""
                    }
                );
                let header_utf16: Vec<u16> = header.encode_utf16().chain(Some(0)).collect();
                unsafe {
                    let _ = SetWindowTextW(self.hwnd_header, PCWSTR(header_utf16.as_ptr()));
                    let _ = ShowWindow(self.hwnd_text, SW_SHOW);
                }
                let mut display = String::new();
                for line in content.lines() {
                    display.push_str(line);
                    display.push_str("\r\n");
                }
                let text_utf16: Vec<u16> = display.encode_utf16().chain(Some(0)).collect();
                unsafe {
                    let _ = SetWindowTextW(self.hwnd_text, PCWSTR(text_utf16.as_ptr()));
                }
            }
            PreviewData::Image {
                mime,
                data_base64: _,
                byte_len,
            } => {
                let header = format!("Image Document\nFormat: {}\nSize: {} bytes", mime, byte_len);
                let header_utf16: Vec<u16> = header.encode_utf16().chain(Some(0)).collect();
                unsafe {
                    let _ = SetWindowTextW(self.hwnd_header, PCWSTR(header_utf16.as_ptr()));
                    let _ = ShowWindow(self.hwnd_text, SW_SHOW);
                }
                let display = format!(
                    "Image format: {}\r\nPayload size: {} bytes\r\n(Native render preview surface)\r\n",
                    mime, byte_len
                );
                let text_utf16: Vec<u16> = display.encode_utf16().chain(Some(0)).collect();
                unsafe {
                    let _ = SetWindowTextW(self.hwnd_text, PCWSTR(text_utf16.as_ptr()));
                }
            }
            PreviewData::Folder { item_count } => {
                let count_str = if let Some(count) = item_count {
                    format!("{} items", count)
                } else {
                    "10,000+ items (capped limit)".to_string()
                };
                let header = format!("Folder Summary\nContents: {}", count_str);
                let header_utf16: Vec<u16> = header.encode_utf16().chain(Some(0)).collect();
                unsafe {
                    let _ = SetWindowTextW(self.hwnd_header, PCWSTR(header_utf16.as_ptr()));
                    let _ = ShowWindow(self.hwnd_text, SW_SHOW);
                }
                let display = format!("Folder:\r\nContains: {}\r\n", count_str);
                let text_utf16: Vec<u16> = display.encode_utf16().chain(Some(0)).collect();
                unsafe {
                    let _ = SetWindowTextW(self.hwnd_text, PCWSTR(text_utf16.as_ptr()));
                }
            }
            PreviewData::Unsupported { reason } => {
                let header = "Preview Unsupported".to_string();
                let header_utf16: Vec<u16> = header.encode_utf16().chain(Some(0)).collect();
                unsafe {
                    let _ = SetWindowTextW(self.hwnd_header, PCWSTR(header_utf16.as_ptr()));
                    let _ = ShowWindow(self.hwnd_text, SW_SHOW);
                }
                let display = format!("Preview unsupported:\r\n{}\r\n", reason);
                let text_utf16: Vec<u16> = display.encode_utf16().chain(Some(0)).collect();
                unsafe {
                    let _ = SetWindowTextW(self.hwnd_text, PCWSTR(text_utf16.as_ptr()));
                }
            }
        }
    }

    pub fn clear(&mut self) {
        unsafe {
            let _ = SetWindowTextW(self.hwnd_header, w!("Select an item to preview"));
            let _ = SetWindowTextW(self.hwnd_text, w!(""));
        }
        self.current_image = None;
    }
}
