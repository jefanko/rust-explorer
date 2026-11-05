use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=resources/app.manifest");
    println!("cargo:rerun-if-changed=resources/app.ico");

    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest_path = manifest_dir.join("resources").join("app.manifest");

    if manifest_path.exists() {
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg=/MANIFESTINPUT:{}",
            manifest_path.display()
        );
    }

    let ico_path = manifest_dir.join("resources").join("app.ico");
    if ico_path.exists() {
        let out_dir = std::env::var_os("OUT_DIR").expect("OUT_DIR");
        let res_path = Path::new(&out_dir).join("app_icon.res");
        compile_ico_to_res(&ico_path, &res_path).expect("compile app.ico into Win32 .res");
        println!("cargo:rustc-link-arg={}", res_path.display());
    }
}

/// Writes a standard 32-bit Windows `.res` file containing `RT_ICON` (type 3) frames
/// and `RT_GROUP_ICON` (type 14, ID 1) directly from `ico_path` without requiring `rc.exe`.
fn compile_ico_to_res(ico_path: &Path, res_path: &Path) -> std::io::Result<()> {
    let ico = std::fs::read(ico_path)?;
    if ico.len() < 6 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "ICO file too short",
        ));
    }
    let count = u16::from_le_bytes([ico[4], ico[5]]) as usize;
    let dir_end = 6 + count * 16;
    if ico.len() < dir_end {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "ICO directory truncated",
        ));
    }

    let mut res = Vec::with_capacity(ico.len() + 256);
    // 32-byte empty initial record required by the Win32 .res format
    write_res_record(&mut res, 0, 0, 0, 0, &[]);

    let mut grp = Vec::with_capacity(6 + count * 14);
    grp.extend_from_slice(&0u16.to_le_bytes()); // idReserved
    grp.extend_from_slice(&1u16.to_le_bytes()); // idType = 1 (ICON)
    grp.extend_from_slice(&(count as u16).to_le_bytes());

    for i in 0..count {
        let entry = &ico[6 + i * 16..6 + (i + 1) * 16];
        let bytes_in_res = u32::from_le_bytes([entry[8], entry[9], entry[10], entry[11]]) as usize;
        let image_offset =
            u32::from_le_bytes([entry[12], entry[13], entry[14], entry[15]]) as usize;
        let image_end = image_offset.saturating_add(bytes_in_res);
        if image_end > ico.len() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "ICO frame out of bounds",
            ));
        }
        let icon_id = (i + 1) as u16;
        // RT_ICON = 3, MemoryFlags = 0x1010 (MOVEABLE | PURE), Language = 0x0409
        write_res_record(
            &mut res,
            3,
            icon_id,
            0x1010,
            0x0409,
            &ico[image_offset..image_end],
        );

        // GRPICONDIRENTRY: first 12 bytes of ICONDIRENTRY + 2-byte nID
        grp.extend_from_slice(&entry[..12]);
        grp.extend_from_slice(&icon_id.to_le_bytes());
    }

    // RT_GROUP_ICON = 14, ID = 1 (default executable icon), MemoryFlags = 0x1030
    write_res_record(&mut res, 14, 1, 0x1030, 0x0409, &grp);
    std::fs::write(res_path, res)
}

fn write_res_record(
    out: &mut Vec<u8>,
    type_id: u16,
    name_id: u16,
    mem_flags: u16,
    lang_id: u16,
    data: &[u8],
) {
    out.extend_from_slice(&(data.len() as u32).to_le_bytes()); // DataSize
    out.extend_from_slice(&32u32.to_le_bytes()); // HeaderSize (32 bytes for ordinal Type + Name)
    out.extend_from_slice(&0xFFFFu16.to_le_bytes());
    out.extend_from_slice(&type_id.to_le_bytes());
    out.extend_from_slice(&0xFFFFu16.to_le_bytes());
    out.extend_from_slice(&name_id.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // DataVersion
    out.extend_from_slice(&mem_flags.to_le_bytes());
    out.extend_from_slice(&lang_id.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // Version
    out.extend_from_slice(&0u32.to_le_bytes()); // Characteristics
    out.extend_from_slice(data);
    while !out.len().is_multiple_of(4) {
        out.push(0);
    }
}
