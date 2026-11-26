//! Build-time catalogue for the exact Cargo resolution, including third-party notices.
//! It runs only while building the application; no Python or network is needed at runtime.
use serde_json::Value;
use std::{env, error::Error, fs, io::Write, path::{Path, PathBuf}, process::Command};

fn main() -> Result<(), Box<dyn Error>> {
    let output_path = env::args_os().nth(1).ok_or("Ausgabedatei fehlt")?;
    let mut command = Command::new(env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
    command.args(["metadata", "--format-version", "1"]);
    if env::var_os("GIRO_LICENSE_OFFLINE").is_some() { command.args(["--locked", "--offline"]); }
    let output = command.output()?;
    if !output.status.success() { return Err(format!("cargo metadata: {}", String::from_utf8_lossy(&output.stderr)).into()); }
    let metadata: Value = serde_json::from_slice(&output.stdout)?;
    let mut packages = metadata["packages"].as_array().ok_or("Cargo-Pakete fehlen")?.clone();
    packages.sort_by_key(|p| format!("{} {}", p["name"].as_str().unwrap_or(""), p["version"].as_str().unwrap_or("")));
    let mut report = fs::File::create(output_path)?;
    writeln!(report, "RUST-ABHÄNGIGKEITEN UND LIZENZHINWEISE\n\nErzeugt aus der Cargo-Auflösung dieses Builds. Die Liste kann auch Bau- oder optionale Abhängigkeiten enthalten; nicht jeder Eintrag ist notwendigerweise im ausführbaren Programm enthalten. Lizenzangaben stammen aus den Metadaten der jeweiligen Pakete.\n")?;
    for p in packages {
        let name = p["name"].as_str().unwrap_or("unbekannt");
        let version = p["version"].as_str().unwrap_or("unbekannt");
        let license = p["license"].as_str().unwrap_or("Keine SPDX-Angabe; Lizenzdatei oder Herkunft prüfen");
        writeln!(report, "\n================================================================\n{name} {version}\nLizenz: {license}")?;
        if let Some(repo) = p["repository"].as_str() { writeln!(report, "Projekt: {repo}")?; }
        if let Some(authors) = p["authors"].as_array() {
            let names: Vec<_> = authors.iter().filter_map(Value::as_str).collect();
            if !names.is_empty() { writeln!(report, "Urheber laut Paket: {}", names.join(", "))?; }
        }
        let manifest = p["manifest_path"].as_str().ok_or("Paketpfad fehlt")?;
        let dir = Path::new(manifest).parent().ok_or("Paketverzeichnis fehlt")?;
        let mut notices: Vec<PathBuf> = Vec::new();
        if let Some(file) = p["license_file"].as_str() {
            let path = dir.join(file);
            if path.is_file() { notices.push(path); }
        }
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
            let name = path.file_name().unwrap_or_default().to_string_lossy().to_ascii_uppercase();
            if path.is_file() && ["LICENSE", "LICENCE", "COPYING", "NOTICE", "COPYRIGHT"].iter().any(|prefix| name.starts_with(prefix)) && !notices.contains(&path) {
                notices.push(path);
            }
        }
        notices.sort();
        if notices.is_empty() { writeln!(report, "Kein Lizenztext im Paket gefunden; bitte beim Hersteller prüfen.")?; }
        for path in notices {
            let filename = path.file_name().unwrap_or_default().to_string_lossy();
            let bytes = fs::read(&path)?;
            writeln!(report, "\n--- {filename} ---\n{}", String::from_utf8_lossy(&bytes))?;
        }
    }
    Ok(())
}
