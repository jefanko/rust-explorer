mod bench;
mod doctor;
mod fixtures;
mod smoke;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("doctor") => doctor::run(),
        Some("fixtures") => {
            let profile = args.next();
            fixtures::run(profile.as_deref())
        }
        Some("bench") => {
            let profile = args.next();
            bench::run(profile.as_deref())
        }
        Some("smoke-installer") => {
            let path = args.next();
            smoke::run(path.as_deref())
        }
        Some(cmd) => {
            eprintln!("Unknown xtask command: {cmd}");
            std::process::exit(1);
        }
        None => {
            eprintln!("Usage: cargo xtask <doctor|fixtures|bench|smoke-installer>");
            std::process::exit(1);
        }
    }
}
