use std::path::{Path, PathBuf};

fn collect_c_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", dir.display()))
        .filter_map(|res| res.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|ext| ext == "c"))
        .collect();
    entries.sort();
    out.extend(entries);
}

fn main() {
    let opus_dir = Path::new("vendor/opus");
    println!("cargo:rerun-if-changed={}", opus_dir.display());

    let mut sources = Vec::new();
    collect_c_files(&opus_dir.join("celt"), &mut sources);
    collect_c_files(&opus_dir.join("silk"), &mut sources);
    collect_c_files(&opus_dir.join("silk/float"), &mut sources);
    for core_file in [
        "opus.c",
        "opus_encoder.c",
        "opus_decoder.c",
        "repacketizer.c",
        "extensions.c",
        "analysis.c",
        "mlp.c",
        "mlp_data.c",
    ] {
        sources.push(opus_dir.join("src").join(core_file));
    }

    let mut build = cc::Build::new();
    build
        .include(opus_dir.join("include"))
        .include(opus_dir.join("celt"))
        .include(opus_dir.join("silk"))
        .include(opus_dir.join("silk/float"))
        .include(opus_dir.join("src"))
        .define("OPUS_BUILD", None)
        .define("USE_ALLOCA", None)
        .define("HAVE_LRINTF", None)
        .define("HAVE_LRINT", None)
        .define("FLOAT_APPROX", None)
        .define("DISABLE_DEBUG_FLOAT", None)
        .define("ENABLE_HARDENING", None)
        .define("PACKAGE_VERSION", "\"1.5.2\"")
        .warnings(false);

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os != "windows" {
        build.define("HAVE_ALLOCA_H", None);
    }
    if target_env == "msvc" {
        build.define("_CRT_SECURE_NO_WARNINGS", None);
    }

    build.files(&sources);
    build.compile("opus");
}
