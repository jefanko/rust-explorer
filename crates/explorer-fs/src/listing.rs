use explorer_domain::models::{EntryKind, FileEntry, SortColumn, SortDirection};
use std::cmp::Ordering;

/// Compares two strings using natural numeric ordering (e.g. "file2" < "file10").
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let mut a_chars = a.chars().peekable();
    let mut b_chars = b.chars().peekable();

    while let (Some(&ac), Some(&bc)) = (a_chars.peek(), b_chars.peek()) {
        if ac.is_ascii_digit() && bc.is_ascii_digit() {
            // Extract numeric sequence from both
            let mut a_num: u64 = 0;
            while let Some(&c) = a_chars.peek() {
                if let Some(digit) = c.to_digit(10) {
                    a_num = a_num.saturating_mul(10).saturating_add(digit as u64);
                    a_chars.next();
                } else {
                    break;
                }
            }

            let mut b_num: u64 = 0;
            while let Some(&c) = b_chars.peek() {
                if let Some(digit) = c.to_digit(10) {
                    b_num = b_num.saturating_mul(10).saturating_add(digit as u64);
                    b_chars.next();
                } else {
                    break;
                }
            }

            match a_num.cmp(&b_num) {
                Ordering::Equal => continue,
                other => return other,
            }
        } else {
            // Case-insensitive character comparison
            let ac_lower = ac.to_lowercase().to_string();
            let bc_lower = bc.to_lowercase().to_string();
            match ac_lower.cmp(&bc_lower) {
                Ordering::Equal => {
                    a_chars.next();
                    b_chars.next();
                }
                other => return other,
            }
        }
    }

    // If one ended before the other
    a.len().cmp(&b.len())
}

/// Sorts file entries: directories first, then by chosen column, then natural name tie-breaker.
pub fn sort_entries(entries: &mut [FileEntry], column: SortColumn, direction: SortDirection) {
    entries.sort_by(|a, b| {
        // Rule 1: Directories always come before files
        let a_is_dir = matches!(a.kind, EntryKind::Directory);
        let b_is_dir = matches!(b.kind, EntryKind::Directory);

        match (a_is_dir, b_is_dir) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            _ => {
                // Both are dirs or both are files: apply chosen column sort
                let ord = match column {
                    SortColumn::Name => natural_cmp(&a.display_name, &b.display_name),
                    SortColumn::Type => {
                        let ext_cmp = a.extension.to_lowercase().cmp(&b.extension.to_lowercase());
                        if ext_cmp == Ordering::Equal {
                            natural_cmp(&a.display_name, &b.display_name)
                        } else {
                            ext_cmp
                        }
                    }
                    SortColumn::Size => match (a.size_bytes, b.size_bytes) {
                        (Some(sa), Some(sb)) => sa.cmp(&sb),
                        (Some(_), None) => Ordering::Less,
                        (None, Some(_)) => Ordering::Greater,
                        (None, None) => natural_cmp(&a.display_name, &b.display_name),
                    },
                    SortColumn::Modified => match (a.modified_filetime, b.modified_filetime) {
                        (Some(ma), Some(mb)) => ma.cmp(&mb),
                        (Some(_), None) => Ordering::Less,
                        (None, Some(_)) => Ordering::Greater,
                        (None, None) => natural_cmp(&a.display_name, &b.display_name),
                    },
                };

                let directed_ord = match direction {
                    SortDirection::Ascending => ord,
                    SortDirection::Descending => ord.reverse(),
                };

                // Deterministic tie-breaker
                if directed_ord == Ordering::Equal {
                    a.display_name.cmp(&b.display_name)
                } else {
                    directed_ord
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use explorer_domain::ids::ItemToken;

    fn make_test_entry(name: &str, kind: EntryKind, size: Option<u64>) -> FileEntry {
        FileEntry {
            token: ItemToken::new(),
            parent_token: None,
            display_name: name.to_string(),
            escaped_name_hint: None,
            extension: name.split('.').next_back().unwrap_or("").to_string(),
            kind,
            size_bytes: size,
            modified_filetime: None,
            attributes: 0,
            is_hidden: false,
            is_readonly: false,
            is_system: false,
        }
    }

    #[test]
    fn test_natural_sort_ordering() {
        assert_eq!(natural_cmp("file2.txt", "file10.txt"), Ordering::Less);
        assert_eq!(natural_cmp("file10.txt", "file2.txt"), Ordering::Greater);
        assert_eq!(natural_cmp("file1.txt", "file1.txt"), Ordering::Equal);
    }

    #[test]
    fn test_sort_directories_first() {
        let mut entries = vec![
            make_test_entry("zebra.txt", EntryKind::File, Some(100)),
            make_test_entry("alpha_folder", EntryKind::Directory, None),
            make_test_entry("apple.txt", EntryKind::File, Some(200)),
            make_test_entry("beta_folder", EntryKind::Directory, None),
        ];

        sort_entries(&mut entries, SortColumn::Name, SortDirection::Ascending);

        assert_eq!(entries[0].display_name, "alpha_folder");
        assert_eq!(entries[1].display_name, "beta_folder");
        assert_eq!(entries[2].display_name, "apple.txt");
        assert_eq!(entries[3].display_name, "zebra.txt");
    }
}
