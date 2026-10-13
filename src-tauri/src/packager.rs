use crate::downloader::sha256_file;
use crate::instances::InstanceMeta;
use crate::sync::{default_managed, default_protected, Manifest, ManifestFile, MANIFEST_FORMAT};
use serde::Serialize;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize)]
pub struct ExportResult {
    pub manifest_path: String,
    pub archive_path: String,
    pub warnings: Vec<String>,
}
fn walk_files(root: &Path, folders: &[String]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for folder in folders {
        let dir = root.join(folder);
        if !dir.exists() {
            continue;
        }
        let mut stack = vec![dir.clone()];
        while let Some(current) = stack.pop() {
            if let Ok(entries) = fs::read_dir(&current) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_dir() {
                        stack.push(p);
                    } else if let Ok(rel) = p.strip_prefix(root) {
                        let rel_str = rel.to_string_lossy().replace('\\', "/");
                        if let Ok(hash) = sha256_file(&p) {
                            out.push((rel_str, hash));
                        }
                    }
                }
            }
        }
    }
    out
}

fn now_revision() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    chrono::Local::now().format("%Y%m%d").to_string() + &secs.to_string()
}
fn write_manifest(
    instance_dir: &Path,
    destination_dir: &Path,
    meta: &InstanceMeta,
    files: &[(String, String)],
    protected: &[String],
    managed: &[String],
    archive_url: &str,
    files_base_url: &str,
    ram_min_mb: u32,
    ram_max_mb: u32,
) -> Result<fs::File, String> {
    let base = files_base_url.trim().trim_end_matches('/');
    let manifest_files: Vec<ManifestFile> = files
        .iter()
        .map(|(path, sha)| ManifestFile {
            path: path.clone(),
            sha256: sha.clone(),
            size: fs::metadata(instance_dir.join(path))
                .map(|m| m.len())
                .unwrap_or(0),
            url: if base.is_empty() {
                None
            } else {
                Some(format!("{base}/{path}"))
            },
        })
        .collect();

    let manifest = Manifest {
        format: MANIFEST_FORMAT.to_string(),
        revision: now_revision(),
        name: meta.name.clone(),
        game_version: meta.game_version.clone(),
        loader: meta.loader.clone(),
        loader_version: meta.loader_version.clone(),
        server_version: String::new(),
        archive_url: archive_url.to_string(),
        ram_min_mb,
        ram_max_mb,
        files: manifest_files,
        protected: protected.to_vec(),
        managed: managed.to_vec(),
    };
    let f = fs::File::create(destination_dir.join("manifest.json"))
        .map_err(|e| format!("crear manifest.json: {e}"))?;
    serde_json::to_writer_pretty(
        &mut std::io::BufWriter::new(&f),
        &manifest,
    )
    .map_err(|e| format!("escribir manifest.json: {e}"))?;
    Ok(f)
}
pub fn export_instance(
    instance_dir: &Path,
    destination_dir: &Path,
    archive_url: &str,
    files_base_url: &str,
    extra_folders: &[String],
    ram_min_mb: u32,
    ram_max_mb: u32,
) -> Result<ExportResult, String> {
    fs::create_dir_all(destination_dir).map_err(|e| format!("crear destino: {e}"))?;

    let meta = crate::instances::load_meta(instance_dir);
    let mut protected = default_protected();
    protected.extend(["mods", "config", "resourcepacks"].iter().map(|s| s.to_string()));

    let mut managed = default_managed();
    for f in extra_folders {
        if !managed.contains(f) {
            managed.push(f.clone());
        }
    }

    let mut files = walk_files(instance_dir, &managed);
    let mut warnings = Vec::new();
    for (path, _) in &files {
        for p in &protected {
            if path == p {
                warnings.push(format!("omitido (protegido): {path}"));
            }
        }
    }
    files.retain(|(path, _)| !protected.iter().any(|p| p == path));

    let per_file = !files_base_url.trim().is_empty();
    let archive_url_used = if per_file { "" } else { archive_url };

    let mut manifest_file = write_manifest(
        instance_dir,
        destination_dir,
        &meta,
        &files,
        &protected,
        &managed,
        archive_url_used,
        files_base_url,
        ram_min_mb,
        ram_max_mb,
    )?;
    manifest_file.flush().ok();

    let mut archive_path = String::new();

    if per_file {
        for (rel_path, _) in &files {
            let src = instance_dir.join(rel_path);
            let dst = destination_dir.join("files").join(rel_path);
            if let Some(parent) = dst.parent() {
                fs::create_dir_all(parent).map_err(|e| format!("crear dir: {e}"))?;
            }
            fs::copy(&src, &dst).map_err(|e| format!("copiar {rel_path}: {e}"))?;
        }
        warnings.push(
            "Modo por archivos: sube la carpeta 'files' a la ruta indicada en la base URL".to_string(),
        );
    } else {
        let archive_path_buf = destination_dir.join(format!("PauPack-{}.pau.zip", meta.name));
        let file = fs::File::create(&archive_path_buf).map_err(|e| format!("crear zip: {e}"))?;
        let mut zip = zip::ZipWriter::new(file);
        let options: zip::write::SimpleFileOptions =
            zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("manifest.json", options)
            .map_err(|e| format!("zip start manifest: {e}"))?;
        zip.write_all(fs::read(destination_dir.join("manifest.json"))
            .map_err(|e| format!("leer manifest: {e}"))?
            .as_slice())
            .map_err(|e| format!("zip manifest: {e}"))?;
        for (rel_path, _) in &files {
            let src = instance_dir.join(rel_path);
            let zip_name = format!("files/{}", rel_path.replace('\\', "/"));
            zip.start_file(zip_name, options)
                .map_err(|e| format!("zip start {rel_path}: {e}"))?;
            let bytes = fs::read(&src).map_err(|e| format!("leer {rel_path}: {e}"))?;
            zip.write_all(&bytes).map_err(|e| format!("zip {rel_path}: {e}"))?;
        }
        zip.finish().map_err(|e| format!("finalizar zip: {e}"))?;
        archive_path = archive_path_buf.to_string_lossy().to_string();
    }

    Ok(ExportResult {
        manifest_path: destination_dir.join("manifest.json").to_string_lossy().to_string(),
        archive_path,
        warnings,
    })
}
pub fn export_whitepaper() -> &'static str {
    "1) Exporta la instancia (crea manifest.json + PauPack-*.pau.zip).\n\
     2) Sube los ficheros a GitHub (o Google Drive).\n\
     3) Comparte el enlace directo del manifest.json.\n\
     4) El jugador pulsa 'Actualizar' → ve el diff → 'Aplicar'.\n\
        Cada actualización genera un backup automático con rollback en 'Ajustes'.\n\
\n\
     Actualizar UN solo archivo (ej. un mod) sin re-bajar el pack:\n\
     Opción A (zip): sustituye el archivo local, vuelve a exportar con la misma\n\
        URL de zip y sube manifest.json + zip. Solo ese archivo se reemplaza.\n\
     Opción B (por archivos, recomendado en GitHub): marca 'Modo por archivos',\n\
        pon la base raw del repo (ej.\n\
        https://raw.githubusercontent.com/usr/repo/main/packs/server) y exporta.\n\
        Se genera manifest.json con url por archivo + carpeta 'files/' para subir.\n\
        Para actualizar, sube solo el archivo nuevo + manifest.json: el jugador\n\
        descarga únicamente lo que cambió."
}