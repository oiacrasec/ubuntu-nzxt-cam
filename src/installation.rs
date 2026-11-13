//! Identidade do build e registro da instalação local para atualização explícita.
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

pub const REPOSITORY: &str = "https://github.com/oiacrasec/ubuntu-nzxt-cam.git";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildInfo {
    pub application: String,
    pub version: String,
    pub revision: String,
    pub dirty: bool,
    pub gui: bool,
    pub tray: bool,
    pub nvidia: bool,
}

impl Default for BuildInfo {
    fn default() -> Self {
        Self {
            application: "nzxt-cam".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            revision: env!("NZXT_CAM_REVISION").into(),
            dirty: env!("NZXT_CAM_DIRTY") == "true",
            gui: cfg!(feature = "gui"),
            tray: cfg!(feature = "tray"),
            nvidia: cfg!(feature = "nvidia"),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Installed {
    pub prefix: PathBuf,
    pub build: BuildInfo,
    pub sha256: String,
}

impl Installed {
    pub fn launcher(&self) -> PathBuf {
        self.prefix.join("bin/nzxt-cam")
    }
    pub fn discover() -> Result<Option<Self>> {
        #[cfg(not(unix))]
        {
            Ok(None)
        }
        #[cfg(unix)]
        {
            let exe = std::env::current_exe()?;
            let marker = exe.with_file_name("nzxt-cam.installation.json");
            if !marker.exists() {
                return Ok(None);
            }
            if fs::metadata(&marker)?.len() > 16 * 1024 {
                bail!("Registro de instalação inválido");
            }
            let installed: Self = serde_json::from_slice(&fs::read(marker)?)?;
            if !installed.prefix.is_absolute()
                || installed.prefix.parent().is_none()
                || installed.build.application != "nzxt-cam"
                || installed.launcher().canonicalize()? != exe.canonicalize()?
                || checksum(&exe)? != installed.sha256
            {
                bail!("Registro de instalação incompatível com este executável");
            }
            Ok(Some(installed))
        }
    }
}

pub fn checksum(path: &Path) -> Result<String> {
    let output = Command::new("sha256sum")
        .arg("--")
        .arg(path)
        .output()
        .context("sha256sum não está disponível")?;
    if !output.status.success() {
        bail!("Falha ao verificar a integridade do executável");
    }
    let hash = String::from_utf8(output.stdout)?
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_owned();
    if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        bail!("Checksum inválido");
    }
    Ok(hash)
}

pub fn register(prefix: &Path) -> Result<()> {
    if !cfg!(unix) {
        bail!("A instalação operacional só está disponível no Linux");
    }
    if !prefix.is_absolute() || prefix.parent().is_none() {
        bail!("Prefixo de instalação inválido");
    }
    let prefix = prefix.canonicalize()?;
    if prefix.parent().is_none() {
        bail!("Prefixo de instalação inválido");
    }
    let exe = std::env::current_exe()?.canonicalize()?;
    if prefix.join("bin/nzxt-cam").canonicalize()? != exe {
        bail!("Execute o registro pelo binário instalado");
    }
    let installed = Installed {
        prefix,
        build: BuildInfo::default(),
        sha256: checksum(&exe)?,
    };
    let temporary = exe.with_file_name(".nzxt-cam.installation.json.new");
    fs::write(&temporary, serde_json::to_vec_pretty(&installed)?)?;
    fs::rename(temporary, exe.with_file_name("nzxt-cam.installation.json"))?;
    Ok(())
}
