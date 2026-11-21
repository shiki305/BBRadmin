use std::env;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    // 1. Embed app.manifest into BBRadmin.exe on MSVC Windows builds.
    let app_manifest = manifest_dir.join("app.manifest");
    println!("cargo:rerun-if-changed={}", app_manifest.display());
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg=/MANIFESTINPUT:{}",
            app_manifest.display()
        );
    }

    // 2. Pack vendor/ (the BBSC 1.0.0 Windows co-op server + GPL license) into OUT_DIR/server.zip
    //    so it travels inside the single BBRadmin.exe binary.
    let vendor_dir = manifest_dir.join("vendor");
    let shadnet_exe = vendor_dir.join("server").join("shadnet.exe");
    if !shadnet_exe.is_file() {
        panic!(
            "vendor/server/shadnet.exe is missing: the co-op server files are not in this checkout"
        );
    }

    let mut files = Vec::new();
    collect_files(&vendor_dir, &vendor_dir, &mut files);
    files.sort();

    for (_, abs) in &files {
        println!("cargo:rerun-if-changed={}", abs.display());
    }
    println!("cargo:rerun-if-changed={}", vendor_dir.display());

    let zip_path = out_dir.join("server.zip");
    let zip_file = File::create(&zip_path).expect("failed to create OUT_DIR/server.zip");
    let mut writer = ZipWriter::new(zip_file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    let mut buf = Vec::new();
    for (rel, abs) in &files {
        writer
            .start_file(rel, options)
            .unwrap_or_else(|e| panic!("failed to start zip entry {rel}: {e}"));
        buf.clear();
        File::open(abs)
            .unwrap_or_else(|e| panic!("failed to open {}: {e}", abs.display()))
            .read_to_end(&mut buf)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", abs.display()));
        writer
            .write_all(&buf)
            .unwrap_or_else(|e| panic!("failed to write zip entry {rel}: {e}"));
    }
    writer.finish().expect("failed to finish server.zip");
}

fn collect_files(root: &Path, dir: &Path, out: &mut Vec<(String, PathBuf)>) {
    let entries = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("failed to read directory {}: {e}", dir.display()));
    for entry in entries {
        let entry = entry.expect("failed to read dir entry");
        let path = entry.path();
        if path.is_dir() {
            collect_files(root, &path, out);
        } else if path.is_file() {
            let rel = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            out.push((rel, path));
        }
    }
}
