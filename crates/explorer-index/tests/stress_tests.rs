use explorer_index::db::{IndexDb, IndexEntryRecord};
use explorer_index::query::QueryEngine;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tempfile::tempdir;

#[test]
fn test_100k_fts5_indexing_and_query_performance() {
    let dir = tempdir().unwrap();
    std::fs::write(
        dir.path().join(".rust-explorer-fixture-root"),
        "synthetic index fixture",
    )
    .unwrap();
    let db_path = dir.path().join("stress_index.sqlite3");

    let db = Arc::new(IndexDb::open(&db_path).expect("open index db"));
    let root = db
        .add_root(&PathBuf::from("C:\\BenchmarkRoot"))
        .expect("add root");

    println!("Inserting 100,000 synthetic entries in batches of 1,000...");
    let start_insert = Instant::now();

    let extensions = ["txt", "pdf", "rs", "png", "zip", "json", "docx", "exe"];

    for batch_idx in 0..100 {
        let mut batch = Vec::with_capacity(1_000);
        for i in 0..1_000 {
            let item_num = batch_idx * 1_000 + i;
            let ext = extensions[item_num % extensions.len()];
            let name = if item_num % 100 == 0 {
                format!("annual_report_{item_num:06}.{ext}")
            } else if item_num % 50 == 0 {
                format!("invoice_september_{item_num:06}.{ext}")
            } else {
                format!("document_data_{item_num:06}.{ext}")
            };

            let file_path = PathBuf::from(format!(
                "C:\\BenchmarkRoot\\subfolder_{}\\{}",
                item_num % 500,
                name
            ));

            batch.push(IndexEntryRecord {
                root_id: root.id.clone(),
                parent_id: None,
                path: file_path,
                name_display: name.clone(),
                name_norm: name.to_lowercase(),
                extension_norm: ext.to_string(),
                kind: 0,
                size_bytes: Some((item_num as u64) * 2048),
                modified_filetime: Some(133500000000000000 + (item_num as u64) * 5000),
                attributes: 32,
                seen_epoch: 1,
            });
        }

        db.batch_upsert_entries(&batch).expect("batch insert");
    }

    let insert_time = start_insert.elapsed();
    println!(
        "Inserted 100,000 entries into SQLite FTS5 in {:?} ({:.0} items/sec)",
        insert_time,
        100_000.0 / insert_time.as_secs_f64()
    );

    // Verify database file size is well within the 350 MiB budget
    let metadata = std::fs::metadata(&db_path).expect("db file metadata");
    let file_size_mb = metadata.len() as f64 / (1024.0 * 1024.0);
    println!(
        "Database size on disk for 100,000 entries: {:.2} MiB",
        file_size_mb
    );
    assert!(
        file_size_mb < 350.0,
        "Database size {:.2} MiB exceeds 350 MiB budget",
        file_size_mb
    );

    // Test Search Queries
    let engine = QueryEngine::new(db.clone());

    // Query 1: Single term substring search "invoice"
    let start_q1 = Instant::now();
    let res1 = engine
        .execute_search("invoice", None, 1, 100)
        .expect("search invoice");
    let q1_time = start_q1.elapsed();
    println!(
        "Query 'invoice' found {} matches in {:?}",
        res1.total_matches, q1_time
    );
    assert_eq!(res1.total_matches, 1000); // capped at 1000
    assert!(
        q1_time.as_millis() < 50,
        "FTS query 'invoice' took {:?} (expected < 50ms)",
        q1_time
    );

    // Query 2: Multiple terms "invoice september"
    let start_q2 = Instant::now();
    let res2 = engine
        .execute_search("invoice september", None, 1, 100)
        .expect("search invoice september");
    let q2_time = start_q2.elapsed();
    println!(
        "Query 'invoice september' found {} matches in {:?}",
        res2.total_matches, q2_time
    );
    assert_eq!(res2.total_matches, 1000);
    assert!(
        q2_time.as_millis() < 50,
        "FTS query took {:?} (expected < 50ms)",
        q2_time
    );

    // Query 3: Term + extension filter "report ext:txt"
    let start_q3 = Instant::now();
    let res3 = engine
        .execute_search("report ext:txt", None, 1, 100)
        .expect("search report ext:txt");
    let q3_time = start_q3.elapsed();
    println!(
        "Query 'report ext:txt' found {} matches in {:?}",
        res3.total_matches, q3_time
    );
    assert!(res3.total_matches > 0);
    assert!(
        q3_time.as_millis() < 50,
        "FTS query took {:?} (expected < 50ms)",
        q3_time
    );

    // Query 4: Quoted phrase with exact name
    let start_q4 = Instant::now();
    let res4 = engine
        .execute_search("\"annual_report_000100\"", None, 1, 100)
        .expect("search exact");
    let q4_time = start_q4.elapsed();
    println!(
        "Query exact phrase found {} matches in {:?}",
        res4.total_matches, q4_time
    );
    assert_eq!(res4.total_matches, 1);
    assert!(
        q4_time.as_millis() < 50,
        "FTS query took {:?} (expected < 50ms)",
        q4_time
    );
}
