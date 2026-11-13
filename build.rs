use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().into())
}

fn main() {
    for path in [
        "src",
        "assets",
        "locales",
        "Cargo.toml",
        "Cargo.lock",
        "build.rs",
        "install-desktop-app.sh",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }
    for reference in ["HEAD", "packed-refs"] {
        if let Some(path) = git(&["rev-parse", "--git-path", reference]) {
            println!("cargo:rerun-if-changed={path}");
        }
    }
    if let Some(reference) = git(&["symbolic-ref", "-q", "HEAD"])
        && let Some(path) = git(&["rev-parse", "--git-path", &reference])
    {
        println!("cargo:rerun-if-changed={path}");
    }
    let revision = git(&["rev-parse", "HEAD"]).unwrap_or_else(|| "unknown".into());
    let changes = git(&[
        "status",
        "--porcelain",
        "--",
        "src",
        "assets",
        "locales",
        "Cargo.toml",
        "Cargo.lock",
        "build.rs",
        "install-desktop-app.sh",
    ]);
    println!("cargo:rustc-env=NZXT_CAM_REVISION={revision}");
    println!(
        "cargo:rustc-env=NZXT_CAM_DIRTY={}",
        changes.is_none_or(|text| !text.is_empty())
    );
}
