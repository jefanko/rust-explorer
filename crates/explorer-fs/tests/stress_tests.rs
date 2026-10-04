use explorer_domain::ids::{FolderToken, ItemToken};
use explorer_domain::models::{EntryKind, FileEntry, SortColumn, SortDirection};
use explorer_fs::snapshots::FolderSnapshot;
use std::path::PathBuf;
use std::time::Instant;

fn generate_100k_entries(folder_token: &FolderToken) -> Vec<FileEntry> {
    let mut entries = Vec::with_capacity(100_000);
    let extensions = ["txt", "pdf", "rs", "png", "zip", "json", "docx", "exe"];

    for i in 0..100_000 {
        let is_dir = i % 50 == 0;
        let ext = if is_dir {
            String::new()
        } else {
            extensions[i % extensions.len()].to_string()
        };
        let display_name = if is_dir {
            format!("folder_{i:06}")
        } else {
            format!("file_{i:06}.{ext}")
        };
        let kind = if is_dir {
            EntryKind::Directory
        } else {
            EntryKind::File
        };

        entries.push(FileEntry {
            token: ItemToken::new(),
            parent_token: Some(folder_token.clone()),
            native_name_utf16: display_name.encode_utf16().collect(),
            display_name,
            escaped_name_hint: None,
            kind,
            size_bytes: if is_dir {
                None
            } else {
                Some((i as u64) * 1024)
            },
            modified_filetime: Some(133500000000000000 + (i as u64) * 1000),
            attributes: if is_dir { 0x10 } else { 0x20 },
            is_readonly: false,
            is_hidden: false,
            is_system: false,
            extension: ext,
        });
    }

    entries
}

#[test]
fn test_100k_directory_listing_and_cached_pagination() {
    let folder_token = FolderToken::new();
    let path = PathBuf::from("C:\\SyntheticStressTest");

    let start_gen = Instant::now();
    let entries = generate_100k_entries(&folder_token);
    println!("Generated 100,000 entries in {:?}", start_gen.elapsed());
    assert_eq!(entries.len(), 100_000);

    let start_snap = Instant::now();
    let snapshot = FolderSnapshot::new(
        folder_token,
        path.clone(),
        path.to_string_lossy().to_string(),
        1,
        entries,
    );
    let snap_time = start_snap.elapsed();
    println!("Snapshot creation for 100,000 entries took {:?}", snap_time);
    assert!(
        snap_time.as_secs_f64() < 1.0,
        "Snapshot creation must be fast"
    );

    // First page request: triggers initial sort & caches the result
    let start_p1 = Instant::now();
    let (p1, total) = snapshot.get_page(0, 50, SortColumn::Name, SortDirection::Ascending);
    let p1_time = start_p1.elapsed();
    println!("Initial 100k sort + page 1 (50 items) took {:?}", p1_time);
    assert_eq!(total, 100_000);
    assert_eq!(p1.len(), 50);
    // Directories must come first
    assert_eq!(p1[0].kind, EntryKind::Directory);
    let max_sort_time = if cfg!(debug_assertions) { 2500 } else { 300 };
    assert!(
        p1_time.as_millis() < max_sort_time,
        "Initial 100k sort took {}ms (max: {}ms)",
        p1_time.as_millis(),
        max_sort_time
    );

    // Subsequent page requests: must use cache and be instant (< 5ms)
    let start_p2 = Instant::now();
    let (p2, _) = snapshot.get_page(50, 50, SortColumn::Name, SortDirection::Ascending);
    let p2_time = start_p2.elapsed();
    println!("Subsequent page 2 (50 items) took {:?}", p2_time);
    assert_eq!(p2.len(), 50);
    assert!(
        p2_time.as_millis() < 5,
        "Subsequent page using cache must be < 5ms"
    );

    // Far-away page (items 50,000 to 50,050)
    let start_far = Instant::now();
    let (p_far, _) = snapshot.get_page(50_000, 50, SortColumn::Name, SortDirection::Ascending);
    let p_far_time = start_far.elapsed();
    println!("Far page at offset 50,000 took {:?}", p_far_time);
    assert_eq!(p_far.len(), 50);
    assert!(
        p_far_time.as_millis() < 5,
        "Far page using cache must be < 5ms"
    );

    // Reverse sort
    let start_rev = Instant::now();
    let (p_rev, _) = snapshot.get_page(0, 50, SortColumn::Name, SortDirection::Descending);
    let p_rev_time = start_rev.elapsed();
    println!("Reverse sort 100k entries took {:?}", p_rev_time);
    assert_eq!(p_rev.len(), 50);
    assert!(
        p_rev_time.as_millis() < max_sort_time,
        "Reverse sort took {}ms (max: {}ms)",
        p_rev_time.as_millis(),
        max_sort_time
    );
}

#[test]
fn test_100k_token_resolution_performance() {
    let folder_token = FolderToken::new();
    let path = PathBuf::from("C:\\SyntheticStressTest");
    let entries = generate_100k_entries(&folder_token);
    let sample_tokens: Vec<ItemToken> = entries
        .iter()
        .step_by(100)
        .map(|e| e.token.clone())
        .collect();

    let snapshot = FolderSnapshot::new(
        folder_token,
        path.clone(),
        path.to_string_lossy().to_string(),
        1,
        entries,
    );

    let start_lookups = Instant::now();
    for token in &sample_tokens {
        let resolved = snapshot.resolve_item(token);
        assert!(resolved.is_some());
    }
    let elapsed = start_lookups.elapsed();
    println!("Resolved {} tokens in {:?}", sample_tokens.len(), elapsed);
    let per_lookup = elapsed / (sample_tokens.len() as u32);
    println!("Average lookup time: {:?}", per_lookup);
    assert!(
        per_lookup.as_micros() < 50,
        "Average lookup must be < 50 microseconds"
    );
}
