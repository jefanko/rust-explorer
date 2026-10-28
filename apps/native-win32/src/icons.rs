//! Shell icon helpers backed by the system small image list.

#![allow(unsafe_op_in_unsafe_fn)]

use windows::Win32::Storage::FileSystem::{FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_NORMAL};
use windows::Win32::UI::Shell::{
    SHFILEINFOW, SHGFI_SMALLICON, SHGFI_SYSICONINDEX, SHGFI_USEFILEATTRIBUTES, SHGetFileInfoW,
};
use windows::core::{PCWSTR, w};

/// Returns the shell's system small image list handle (0 when unavailable).
///
/// The list is owned by the shell: never destroy it. It stays valid for the
/// process lifetime, so controls can simply reference it.
pub fn system_small_image_list() -> isize {
    let mut info = SHFILEINFOW::default();
    let result = unsafe {
        SHGetFileInfoW(
            w!("folder"),
            FILE_ATTRIBUTE_DIRECTORY,
            Some(&mut info),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_SYSICONINDEX | SHGFI_SMALLICON | SHGFI_USEFILEATTRIBUTES,
        )
    };
    result as isize
}

fn query(
    path: &str,
    attributes: windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES,
    use_file_attributes: bool,
) -> i32 {
    let wide: Vec<u16> = path.encode_utf16().chain(Some(0)).collect();
    let mut flags = SHGFI_SYSICONINDEX | SHGFI_SMALLICON;
    if use_file_attributes {
        flags |= SHGFI_USEFILEATTRIBUTES;
    }
    let mut info = SHFILEINFOW::default();
    let result = unsafe {
        SHGetFileInfoW(
            PCWSTR(wide.as_ptr()),
            attributes,
            Some(&mut info),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            flags,
        )
    };
    if result == 0 { 0 } else { info.iIcon }
}

/// Icon index for a directory, a file extension, or a special node.
pub fn icon_index(key: &str, is_dir: bool) -> i32 {
    if is_dir {
        return query("folder", FILE_ATTRIBUTE_DIRECTORY, true);
    }
    match key {
        "drive" => query("C:\\", FILE_ATTRIBUTE_DIRECTORY, false),
        "favorite" => query("favorite", FILE_ATTRIBUTE_DIRECTORY, true),
        "file" | "link" => query("file", FILE_ATTRIBUTE_NORMAL, true),
        extension => query(&format!("file.{extension}"), FILE_ATTRIBUTE_NORMAL, true),
    }
}
