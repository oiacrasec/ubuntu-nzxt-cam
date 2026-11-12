use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

pub const NZXT_VENDOR: u16 = 0x1e71;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// Kraken X52/X62/X72 (`KrakenX526272` no CAM).
    KrakenX,
    /// Smart Device V1.
    SmartDevice,
}

impl Kind {
    pub const ALL: [Self; 2] = [Self::KrakenX, Self::SmartDevice];

    pub fn product(self) -> u16 {
        match self {
            Self::KrakenX => 0x170e,
            Self::SmartDevice => 0x1714,
        }
    }

    pub fn from_product(product: u16) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.product() == product)
    }

    /// Nomes do catálogo `devices.json` do CAM.
    pub fn display_name(self) -> &'static str {
        match self {
            Self::KrakenX => "Kraken X",
            Self::SmartDevice => "Smart Device",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Self::KrakenX => "kraken_x",
            Self::SmartDevice => "smart_device",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Info {
    pub kind: Kind,
    /// Identidade estável entre reinícios: tipo + serial, ou caminho USB.
    pub id: String,
    pub node: PathBuf,
    pub serial: String,
    pub phys: String,
}

/// Lê `HID_ID=0003:00001E71:0000170E` e campos vizinhos de um `uevent`.
fn parse_uevent(text: &str) -> Option<(u16, u16, String, String)> {
    let mut ids = None;
    let (mut serial, mut phys) = (String::new(), String::new());
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("HID_ID=") {
            let mut parts = value.split(':');
            let _bus = parts.next()?;
            let vendor = u32::from_str_radix(parts.next()?, 16).ok()?;
            let product = u32::from_str_radix(parts.next()?, 16).ok()?;
            ids = Some((u16::try_from(vendor).ok()?, u16::try_from(product).ok()?));
        } else if let Some(value) = line.strip_prefix("HID_UNIQ=") {
            serial = value.trim().to_owned();
        } else if let Some(value) = line.strip_prefix("HID_PHYS=") {
            phys = value.trim().to_owned();
        }
    }
    let (vendor, product) = ids?;
    Some((vendor, product, serial, phys))
}

/// Dispositivos suportados presentes em `<sys>/class/hidraw`.
pub fn discover(sys: &Path, dev: &Path) -> Vec<Info> {
    let mut found = Vec::new();
    let Ok(entries) = fs::read_dir(sys.join("class/hidraw")) else {
        return found;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Ok(text) = fs::read_to_string(entry.path().join("device/uevent")) else {
            continue;
        };
        let Some((vendor, product, serial, phys)) = parse_uevent(&text) else {
            continue;
        };
        let Some(kind) = (vendor == NZXT_VENDOR)
            .then(|| Kind::from_product(product))
            .flatten()
        else {
            continue;
        };
        let identity = if serial.is_empty() { &phys } else { &serial };
        found.push(Info {
            kind,
            id: format!("{}:{}", kind.slug(), identity),
            node: dev.join(&name),
            serial,
            phys,
        });
    }
    found.sort_by(|a, b| a.id.cmp(&b.id));
    found
}

/// Processos do mesmo usuário com o nó aberto, como o aviso de conflito do CAM.
pub fn holders(proc_: &Path, node: &Path) -> Vec<(u32, String)> {
    let mut out = Vec::new();
    let me = std::process::id();
    let Ok(entries) = fs::read_dir(proc_) else {
        return out;
    };
    for entry in entries.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|s| s.parse::<u32>().ok())
        else {
            continue;
        };
        if pid == me {
            continue;
        }
        let Ok(fds) = fs::read_dir(entry.path().join("fd")) else {
            continue;
        };
        if fds
            .flatten()
            .any(|fd| fs::read_link(fd.path()).is_ok_and(|target| target == node))
        {
            let comm = fs::read_to_string(entry.path().join("comm"))
                .map(|s| s.trim().to_owned())
                .unwrap_or_default();
            out.push((pid, comm));
        }
    }
    out
}

/// Canal de relatórios HID. O primeiro byte de cada escrita é o número do relatório.
pub trait Transport: Send {
    fn write(&mut self, report: &[u8]) -> Result<()>;
    /// Lê um relatório de entrada; `Ok(0)` significa tempo esgotado.
    fn read(&mut self, buf: &mut [u8], timeout: Duration) -> Result<usize>;
}

#[cfg(unix)]
pub struct Hidraw {
    file: fs::File,
}

#[cfg(unix)]
impl Hidraw {
    pub fn open(node: &Path) -> Result<Self> {
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(node)
            .with_context(|| match fs::metadata(node) {
                Ok(_) => format!(
                    "Sem permissão para {}; prepare as permissões em Configurações",
                    node.display()
                ),
                Err(_) => format!("Dispositivo {} desconectado", node.display()),
            })?;
        Ok(Self { file })
    }
}

#[cfg(unix)]
impl Transport for Hidraw {
    fn write(&mut self, report: &[u8]) -> Result<()> {
        use std::io::Write;
        let written = self.file.write(report)?;
        if written != report.len() {
            bail!(
                "Escrita HID incompleta: {written} de {} bytes",
                report.len()
            );
        }
        Ok(())
    }

    fn read(&mut self, buf: &mut [u8], timeout: Duration) -> Result<usize> {
        use nix::poll::{PollFd, PollFlags, PollTimeout, poll};
        use std::{io::Read, os::fd::AsFd};
        let millis = u16::try_from(timeout.as_millis()).unwrap_or(u16::MAX);
        let mut fds = [PollFd::new(self.file.as_fd(), PollFlags::POLLIN)];
        if poll(&mut fds, PollTimeout::from(millis))? == 0 {
            return Ok(0);
        }
        Ok(self.file.read(buf)?)
    }
}

#[cfg(not(unix))]
pub struct Hidraw;

#[cfg(not(unix))]
impl Hidraw {
    pub fn open(_: &Path) -> Result<Self> {
        bail!("hidraw só existe no Linux")
    }
}

#[cfg(not(unix))]
impl Transport for Hidraw {
    fn write(&mut self, _: &[u8]) -> Result<()> {
        bail!("hidraw só existe no Linux")
    }
    fn read(&mut self, _: &mut [u8], _: Duration) -> Result<usize> {
        bail!("hidraw só existe no Linux")
    }
}

/// Transporte simulado: registra escritas e devolve respostas enfileiradas.
#[cfg(test)]
#[derive(Clone, Default)]
pub struct Mock {
    pub writes: std::sync::Arc<std::sync::Mutex<Vec<Vec<u8>>>>,
    pub reads: std::sync::Arc<std::sync::Mutex<std::collections::VecDeque<Vec<u8>>>>,
    pub fail_writes: bool,
}

#[cfg(test)]
impl Mock {
    pub fn queue(&self, report: &[u8]) {
        self.reads.lock().unwrap().push_back(report.to_vec());
    }
    pub fn written(&self) -> Vec<Vec<u8>> {
        self.writes.lock().unwrap().clone()
    }
}

#[cfg(test)]
impl Transport for Mock {
    fn write(&mut self, report: &[u8]) -> Result<()> {
        if self.fail_writes {
            bail!("falha simulada");
        }
        self.writes.lock().unwrap().push(report.to_vec());
        Ok(())
    }
    fn read(&mut self, buf: &mut [u8], _: Duration) -> Result<usize> {
        let Some(report) = self.reads.lock().unwrap().pop_front() else {
            return Ok(0);
        };
        let n = report.len().min(buf.len());
        buf[..n].copy_from_slice(&report[..n]);
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sensors::tests::write;

    fn hidraw(root: &Path, name: &str, id: &str, uniq: &str) {
        write(
            root,
            &format!("sys/class/hidraw/{name}/device/uevent"),
            &format!(
                "DRIVER=hid-generic\nHID_ID={id}\nHID_NAME=NZXT USB Device\nHID_PHYS=usb-0000:00:14.0-9/input0\nHID_UNIQ={uniq}\n"
            ),
        );
    }

    #[test]
    fn discovers_supported_devices_only() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        hidraw(root, "hidraw3", "0003:00001E71:0000170E", "6D8B4469545");
        hidraw(root, "hidraw4", "0003:00001E71:00001714", "876F27655052");
        hidraw(root, "hidraw5", "0003:00001E71:00002007", "X3");
        hidraw(root, "hidraw6", "0003:00000B05:000018A3", "AURA");
        write(
            root,
            "sys/class/hidraw/hidraw7/device/uevent",
            "HID_ID=garbage\n",
        );
        let found = discover(&root.join("sys"), Path::new("/dev"));
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].kind, Kind::KrakenX);
        assert_eq!(found[0].id, "kraken_x:6D8B4469545");
        assert_eq!(found[0].node, Path::new("/dev/hidraw3"));
        assert_eq!(found[1].kind, Kind::SmartDevice);
    }

    #[test]
    fn missing_serial_uses_usb_path() {
        let dir = tempfile::tempdir().unwrap();
        hidraw(dir.path(), "hidraw0", "0003:00001E71:00001714", "");
        let found = discover(&dir.path().join("sys"), Path::new("/dev"));
        assert_eq!(found[0].id, "smart_device:usb-0000:00:14.0-9/input0");
    }

    #[test]
    fn no_sysfs_means_no_devices() {
        let dir = tempfile::tempdir().unwrap();
        assert!(discover(&dir.path().join("sys"), Path::new("/dev")).is_empty());
        assert!(holders(&dir.path().join("proc"), Path::new("/dev/hidraw0")).is_empty());
    }
}
