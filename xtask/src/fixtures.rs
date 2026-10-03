use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

pub fn run(profile: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let count: usize = match profile {
        Some("small") => 100,
        Some("medium") => 1_000,
        Some("large") => 10_000,
        _ => 500,
    };

    let target_dir = Path::new("target").join("fixtures").join("sample_tree");
    fs::create_dir_all(&target_dir)?;

    println!(
        "Generating {count} sample fixture files in {}...",
        target_dir.display()
    );
    for i in 0..count {
        let is_dir = i % 20 == 0;
        if is_dir {
            fs::create_dir_all(target_dir.join(format!("dir_{i:04}")))?;
        } else {
            let mut f = File::create(target_dir.join(format!("sample_file_{i:04}.txt")))?;
            writeln!(f, "Sample content for fixture file {i}")?;
        }
    }

    println!("[OK] Fixtures generated successfully.");
    Ok(())
}
