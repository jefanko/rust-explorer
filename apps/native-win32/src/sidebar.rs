#![allow(dead_code, unsafe_op_in_unsafe_fn)]

use explorer_domain::models::{DriveItem, KnownFolderItem};
use std::collections::HashMap;
use std::path::PathBuf;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::Controls::*;
use windows::Win32::UI::WindowsAndMessaging::SendMessageW;
use windows::core::PWSTR;

pub struct SidebarManager {
    pub hwnd_tree: HWND,
    pub item_paths: HashMap<isize, PathBuf>,
    image_list_ready: bool,
}

impl SidebarManager {
    pub fn new(hwnd_tree: HWND) -> Self {
        Self {
            hwnd_tree,
            item_paths: HashMap::new(),
            image_list_ready: false,
        }
    }

    fn ensure_image_list(&mut self) {
        if self.image_list_ready {
            return;
        }
        let image_list = crate::icons::system_small_image_list();
        if image_list == 0 {
            return;
        }
        unsafe {
            let _ = SendMessageW(
                self.hwnd_tree,
                TVM_SETIMAGELIST,
                WPARAM(TVSIL_NORMAL as usize),
                LPARAM(image_list),
            );
        }
        self.image_list_ready = true;
    }

    pub fn populate(
        &mut self,
        known_folders: &[KnownFolderItem],
        drives: &[DriveItem],
        favorites: &[String],
    ) {
        self.ensure_image_list();
        unsafe {
            let _ = SendMessageW(self.hwnd_tree, TVM_SETITEMHEIGHT, WPARAM(22), LPARAM(0));
            let _ = SendMessageW(
                self.hwnd_tree,
                TVM_DELETEITEM,
                WPARAM(0),
                LPARAM(TVI_ROOT.0 as isize),
            );
        }
        self.item_paths.clear();

        let folder_icon = crate::icons::icon_index("", true);
        let drive_icon = crate::icons::icon_index("drive", false);

        // 1. Quick Access (Known Folders)
        let root_qa = self.insert_node(TVI_ROOT, "Quick Access", None, folder_icon);
        for kf in known_folders {
            let label = match kf.id.as_str() {
                "desktop" => "Desktop",
                "documents" => "Documents",
                "downloads" => "Downloads",
                "music" => "Music",
                "pictures" => "Pictures",
                "videos" => "Videos",
                _ => &kf.name,
            };
            self.insert_node(root_qa, label, Some(PathBuf::from(&kf.path)), folder_icon);
        }
        unsafe {
            let _ = SendMessageW(
                self.hwnd_tree,
                TVM_EXPAND,
                WPARAM(TVE_EXPAND.0 as usize),
                LPARAM(root_qa.0 as isize),
            );
        }

        // 2. This PC (Drives)
        let root_pc = self.insert_node(TVI_ROOT, "This PC", None, drive_icon);
        for drive in drives {
            let label = if let Some(free) = drive.free_bytes {
                let free_gb = free / (1024 * 1024 * 1024);
                format!("{} ({} GB free)", drive.name, free_gb)
            } else {
                drive.name.clone()
            };
            self.insert_node(
                root_pc,
                &label,
                Some(PathBuf::from(&drive.path)),
                drive_icon,
            );
        }
        unsafe {
            let _ = SendMessageW(
                self.hwnd_tree,
                TVM_EXPAND,
                WPARAM(TVE_EXPAND.0 as usize),
                LPARAM(root_pc.0 as isize),
            );
        }

        // 3. Favorites
        let root_fav = self.insert_node(TVI_ROOT, "Favorites", None, folder_icon);
        for fav in favorites {
            let name = PathBuf::from(fav)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or(fav)
                .to_string();
            self.insert_node(root_fav, &name, Some(PathBuf::from(fav)), folder_icon);
        }
        unsafe {
            let _ = SendMessageW(
                self.hwnd_tree,
                TVM_EXPAND,
                WPARAM(TVE_EXPAND.0 as usize),
                LPARAM(root_fav.0 as isize),
            );
        }
    }

    fn insert_node(
        &mut self,
        parent: HTREEITEM,
        label: &str,
        path: Option<PathBuf>,
        icon: i32,
    ) -> HTREEITEM {
        let mut label_utf16: Vec<u16> = label.encode_utf16().chain(Some(0)).collect();
        let mut tvins = TVINSERTSTRUCTW {
            hParent: parent,
            hInsertAfter: TVI_LAST,
            Anonymous: TVINSERTSTRUCTW_0 {
                item: TVITEMW {
                    mask: TVIF_TEXT | TVIF_IMAGE | TVIF_SELECTEDIMAGE,
                    pszText: PWSTR(label_utf16.as_mut_ptr()),
                    iImage: icon,
                    iSelectedImage: icon,
                    ..Default::default()
                },
            },
        };

        let hitem = unsafe {
            let res = SendMessageW(
                self.hwnd_tree,
                TVM_INSERTITEMW,
                WPARAM(0),
                LPARAM(&mut tvins as *mut _ as isize),
            );
            HTREEITEM(res.0)
        };

        if let Some(p) = path {
            self.item_paths.insert(hitem.0 as isize, p);
        }

        hitem
    }

    pub fn get_path_for_item(&self, hitem: HTREEITEM) -> Option<PathBuf> {
        self.item_paths.get(&(hitem.0 as isize)).cloned()
    }
}
