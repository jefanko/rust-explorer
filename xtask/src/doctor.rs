use std::process::Command;

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    println!("Running xtask doctor...");
    let mut all_ok = true;

    // 1. Check Git
    match Command::new("git").arg("--version").output() {
        Ok(out) if out.status.success() => {
            println!(" [OK] git: {}", String::from_utf8_lossy(&out.stdout).trim());
        }
        _ => {
            eprintln!(" [FAIL] git not found or failed");
            all_ok = false;
        }
    }

    // 2. Check Node & npm
    match Command::new("node").arg("--version").output() {
        Ok(out) if out.status.success() => {
            println!(
                " [OK] node: {}",
                String::from_utf8_lossy(&out.stdout).trim()
            );
        }
        _ => {
            eprintln!(" [FAIL] node not found");
            all_ok = false;
        }
    }
    match Command::new("npm.cmd").arg("--version").output() {
        Ok(out) if out.status.success() => {
            println!(" [OK] npm: {}", String::from_utf8_lossy(&out.stdout).trim());
        }
        _ => {
            // Try npm without .cmd
            if let Ok(out) = Command::new("npm").arg("--version").output() {
                if out.status.success() {
                    println!(" [OK] npm: {}", String::from_utf8_lossy(&out.stdout).trim());
                } else {
                    eprintln!(" [FAIL] npm not found");
                    all_ok = false;
                }
            } else {
                eprintln!(" [FAIL] npm not found");
                all_ok = false;
            }
        }
    }

    // 3. Test SQLite FTS5 trigram availability
    println!(" Testing bundled SQLite FTS5 trigram capability...");
    match test_sqlite_fts5_trigram() {
        Ok(()) => println!(" [OK] SQLite bundled with FTS5 trigram support working"),
        Err(e) => {
            eprintln!(" [FAIL] SQLite FTS5 trigram probe failed: {e}");
            all_ok = false;
        }
    }

    if all_ok {
        println!("\nAll doctor checks PASSED.");
        Ok(())
    } else {
        eprintln!("\nDoctor checks FAILED.");
        std::process::exit(1);
    }
}

fn test_sqlite_fts5_trigram() -> Result<(), Box<dyn std::error::Error>> {
    let conn = rusqlite::Connection::open_in_memory()?;
    conn.execute(
        "CREATE VIRTUAL TABLE test_fts USING fts5(name, tokenize='trigram');",
        [],
    )?;
    conn.execute(
        "INSERT INTO test_fts(name) VALUES (?1);",
        ["rust_explorer_document.pdf"],
    )?;
    let mut stmt = conn.prepare("SELECT name FROM test_fts WHERE name MATCH ?1;")?;
    let mut rows = stmt.query(["explorer"])?;
    if let Some(row) = rows.next()? {
        let name: String = row.get(0)?;
        if name == "rust_explorer_document.pdf" {
            return Ok(());
        }
    }
    Err("FTS5 match did not return expected entry".into())
}
