use base64::prelude::*;
use explorer_domain::models::PreviewData;
use std::fs::File;
use std::io::Read;
use std::path::Path;

const MAX_TEXT_BYTES: u64 = 64 * 1024; // 64 KiB
const MAX_IMAGE_BYTES: u64 = 8 * 1024 * 1024; // 8 MiB
const BINARY_SNIFF_BYTES: usize = 8192; // 8 KiB

/// Inspects a filesystem entry and generates preview content.
pub fn build_preview(path: &Path) -> PreviewData {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) => {
            return PreviewData::Unsupported {
                reason: format!("File inaccessible: {e}"),
            };
        }
    };

    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        return PreviewData::Unsupported {
            reason: "Links and reparse points are not previewed directly".to_string(),
        };
    }

    if file_type.is_dir() {
        return preview_folder(path);
    }

    if !file_type.is_file() {
        return PreviewData::Unsupported {
            reason: "Not a regular file".to_string(),
        };
    }

    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();

    if let Some(mime) = image_mime_type(&ext) {
        return preview_image(path, metadata.len(), mime);
    }

    preview_text(path, metadata.len())
}

fn preview_folder(path: &Path) -> PreviewData {
    match std::fs::read_dir(path) {
        Ok(read_dir) => {
            let mut count = 0;
            for entry in read_dir {
                if entry.is_ok() {
                    count += 1;
                    if count >= 10_000 {
                        break;
                    }
                }
            }
            PreviewData::Folder {
                item_count: Some(count),
            }
        }
        Err(_) => PreviewData::Folder { item_count: None },
    }
}

fn image_mime_type(ext: &str) -> Option<&'static str> {
    match ext {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "bmp" => Some("image/bmp"),
        "ico" => Some("image/x-icon"),
        "svg" => Some("image/svg+xml"),
        _ => None,
    }
}

fn preview_image(path: &Path, file_len: u64, mime: &'static str) -> PreviewData {
    if file_len > MAX_IMAGE_BYTES {
        return PreviewData::Unsupported {
            reason: "Image too large to preview (> 8 MB)".to_string(),
        };
    }

    match std::fs::read(path) {
        Ok(bytes) => {
            let data_base64 = BASE64_STANDARD.encode(&bytes);
            PreviewData::Image {
                mime: mime.to_string(),
                data_base64,
                byte_len: bytes.len() as u64,
            }
        }
        Err(e) => PreviewData::Unsupported {
            reason: format!("Failed to read image: {e}"),
        },
    }
}

fn preview_text(path: &Path, file_len: u64) -> PreviewData {
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(e) => {
            return PreviewData::Unsupported {
                reason: format!("Failed to open file: {e}"),
            };
        }
    };

    let mut buf = Vec::new();
    let read_result = (&mut file).take(MAX_TEXT_BYTES).read_to_end(&mut buf);

    if let Err(e) = read_result {
        return PreviewData::Unsupported {
            reason: format!("Failed to read file: {e}"),
        };
    }

    if buf.is_empty() {
        return PreviewData::Text {
            content: String::new(),
            truncated: false,
            encoding: "utf-8".to_string(),
            line_count: 0,
        };
    }

    let truncated = file_len > buf.len() as u64;

    // Detect UTF-16 BOMs
    if buf.len() >= 2 && buf[0] == 0xFF && buf[1] == 0xFE {
        return decode_utf16_le(&buf[2..], truncated);
    }
    if buf.len() >= 2 && buf[0] == 0xFE && buf[1] == 0xFF {
        return decode_utf16_be(&buf[2..], truncated);
    }

    // Binary check: NUL byte in the first 8 KiB
    let check_len = buf.len().min(BINARY_SNIFF_BYTES);
    if buf[..check_len].contains(&0x00) {
        return PreviewData::Unsupported {
            reason: "Binary file cannot be previewed as text".to_string(),
        };
    }

    // Strip UTF-8 BOM if present
    let text_slice = if buf.len() >= 3 && buf[0] == 0xEF && buf[1] == 0xBB && buf[2] == 0xBF {
        &buf[3..]
    } else {
        &buf[..]
    };

    let mut content = String::from_utf8_lossy(text_slice).into_owned();

    // If truncated, prevent trailing replacement character from a sliced multi-byte UTF-8 code point
    if truncated && content.ends_with('\u{FFFD}') {
        content.pop();
    }

    let line_count = content.lines().count();
    PreviewData::Text {
        content,
        truncated,
        encoding: "utf-8".to_string(),
        line_count,
    }
}

fn decode_utf16_le(bytes: &[u8], truncated: bool) -> PreviewData {
    let u16_count = bytes.len() / 2;
    let mut u16_buf = Vec::with_capacity(u16_count);
    for chunk in bytes[..u16_count * 2].as_chunks::<2>().0 {
        u16_buf.push(u16::from_le_bytes([chunk[0], chunk[1]]));
    }
    let content = String::from_utf16_lossy(&u16_buf);
    let line_count = content.lines().count();
    PreviewData::Text {
        content,
        truncated,
        encoding: "utf-16le".to_string(),
        line_count,
    }
}

fn decode_utf16_be(bytes: &[u8], truncated: bool) -> PreviewData {
    let u16_count = bytes.len() / 2;
    let mut u16_buf = Vec::with_capacity(u16_count);
    for chunk in bytes[..u16_count * 2].as_chunks::<2>().0 {
        u16_buf.push(u16::from_be_bytes([chunk[0], chunk[1]]));
    }
    let content = String::from_utf16_lossy(&u16_buf);
    let line_count = content.lines().count();
    PreviewData::Text {
        content,
        truncated,
        encoding: "utf-16be".to_string(),
        line_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn preview_utf8_text_file() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("sample.txt");
        std::fs::write(&file, "Hello world\nSecond line").unwrap();

        match build_preview(&file) {
            PreviewData::Text {
                content,
                truncated,
                encoding,
                line_count,
            } => {
                assert_eq!(content, "Hello world\nSecond line");
                assert!(!truncated);
                assert_eq!(encoding, "utf-8");
                assert_eq!(line_count, 2);
            }
            other => panic!("Unexpected preview: {other:?}"),
        }
    }

    #[test]
    fn preview_utf16le_with_bom() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("utf16.txt");
        let mut data = vec![0xFF, 0xFE]; // LE BOM
        for c in "Unicode Text".encode_utf16() {
            data.extend_from_slice(&c.to_le_bytes());
        }
        std::fs::write(&file, data).unwrap();

        match build_preview(&file) {
            PreviewData::Text {
                content,
                truncated,
                encoding,
                ..
            } => {
                assert_eq!(content, "Unicode Text");
                assert!(!truncated);
                assert_eq!(encoding, "utf-16le");
            }
            other => panic!("Unexpected preview: {other:?}"),
        }
    }

    #[test]
    fn preview_binary_file_returns_unsupported() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("binary.dat");
        let data = vec![0x10, 0x20, 0x00, 0x30, 0x40];
        std::fs::write(&file, data).unwrap();

        match build_preview(&file) {
            PreviewData::Unsupported { reason } => {
                assert!(reason.contains("Binary"));
            }
            other => panic!("Unexpected preview: {other:?}"),
        }
    }

    #[test]
    fn preview_image_base64() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("icon.PNG");
        let raw_bytes = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        std::fs::write(&file, &raw_bytes).unwrap();

        match build_preview(&file) {
            PreviewData::Image {
                mime,
                data_base64,
                byte_len,
            } => {
                assert_eq!(mime, "image/png");
                assert_eq!(byte_len, raw_bytes.len() as u64);
                assert_eq!(data_base64, BASE64_STANDARD.encode(&raw_bytes));
            }
            other => panic!("Unexpected preview: {other:?}"),
        }
    }

    #[test]
    fn preview_folder_counts_children() {
        let dir = tempdir().unwrap();
        let folder = dir.path().join("sub");
        std::fs::create_dir(&folder).unwrap();
        std::fs::write(folder.join("f1.txt"), "a").unwrap();
        std::fs::write(folder.join("f2.txt"), "b").unwrap();

        match build_preview(&folder) {
            PreviewData::Folder { item_count } => {
                assert_eq!(item_count, Some(2));
            }
            other => panic!("Unexpected preview: {other:?}"),
        }
    }

    #[test]
    fn preview_missing_file_returns_unsupported() {
        let dir = tempdir().unwrap();
        let missing = dir.path().join("does_not_exist.txt");

        match build_preview(&missing) {
            PreviewData::Unsupported { reason } => {
                assert!(reason.contains("inaccessible"));
            }
            other => panic!("Unexpected preview: {other:?}"),
        }
    }
}
