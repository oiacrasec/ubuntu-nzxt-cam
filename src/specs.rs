//! Especificações do sistema (tela "Especificações do sistema" do CAM), lidas uma
//! vez de sysfs, `/proc`, do banco do udev (`/run/udev/data`) e de `pci.ids`, sem
//! privilégios. Os módulos de RAM vêm das propriedades `MEMORY_DEVICE_*` que o udev
//! extrai da tabela SMBIOS; sem elas, a RAM aparece como total. Formato de áudio
//! (`pactl`) e endereços IP são aplicados à parte, por [`apply_audio`] e
//! [`apply_addresses`].
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

/// Onde ler as especificações; os testes apontam para diretórios sintéticos.
pub struct Sources {
    pub sys: PathBuf,
    pub proc_: PathBuf,
    /// Raiz de `udev/data` (normalmente `/run`).
    pub run: PathBuf,
    pub pci_ids: PathBuf,
}

impl Default for Sources {
    fn default() -> Self {
        let pci_ids = ["/usr/share/misc/pci.ids", "/usr/share/hwdata/pci.ids"]
            .into_iter()
            .map(PathBuf::from)
            .find(|path| path.exists())
            .unwrap_or_else(|| "/usr/share/misc/pci.ids".into());
        Self {
            sys: "/sys".into(),
            proc_: "/proc".into(),
            run: "/run".into(),
            pci_ids,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CpuDetails {
    pub name: String,
    pub vendor: String,
    pub cores: usize,
    pub threads: usize,
    pub base_mhz: Option<f64>,
    pub max_mhz: Option<f64>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Specs {
    pub cpu: CpuDetails,
    pub motherboard: Option<String>,
    pub memory_total_bytes: Option<u64>,
    pub storage: Vec<String>,
    pub audio: Vec<String>,
    pub network: Vec<String>,
    #[serde(default)]
    pub memory_modules: Vec<String>,
    /// Campos por identidade da lista; dados ausentes são exibidos como indisponíveis.
    #[serde(default)]
    pub details: BTreeMap<String, Vec<(String, String)>>,
    /// Número ALSA de cada item de `audio`, para casar com o `pactl`.
    #[serde(skip)]
    pub audio_cards: Vec<u32>,
    /// Interface de cada item de `network`, para os endereços IP.
    #[serde(skip)]
    pub network_interfaces: Vec<String>,
}

const UNAVAILABLE: &str = "—";

/// Propriedades `E:CHAVE=VALOR` de um registro do banco do udev.
fn udev_properties(path: &Path) -> BTreeMap<String, String> {
    fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| line.strip_prefix("E:")?.split_once('='))
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .collect()
}

/// Desfaz o escape `\xHH` dos valores `*_ENC` do udev.
fn udev_unescape(value: &str) -> String {
    let mut bytes = Vec::new();
    let raw = value.as_bytes();
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == b'\\'
            && raw.get(i + 1) == Some(&b'x')
            && let Some(byte) = value
                .get(i + 2..i + 4)
                .and_then(|hex| u8::from_str_radix(hex, 16).ok())
        {
            bytes.push(byte);
            i += 4;
        } else {
            bytes.push(raw[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&bytes).trim().to_owned()
}

/// Nomes do banco `pci.ids`.
struct PciIds(String);

impl PciIds {
    fn load(path: &Path) -> Self {
        Self(fs::read_to_string(path).unwrap_or_default())
    }

    fn vendor(&self, vendor: u16) -> Option<&str> {
        let prefix = format!("{vendor:04x}  ");
        self.0
            .lines()
            .find_map(|line| line.strip_prefix(&prefix))
            .map(str::trim)
    }

    fn device(&self, vendor: u16, device: u16) -> Option<&str> {
        let vendor_line = format!("{vendor:04x}  ");
        let device_line = format!("\t{device:04x}  ");
        let mut inside = false;
        for line in self.0.lines() {
            if line.starts_with(&vendor_line) {
                inside = true;
            } else if inside && !line.starts_with('\t') && !line.starts_with('#') {
                return None;
            } else if inside && let Some(name) = line.strip_prefix(&device_line) {
                return Some(name.trim());
            }
        }
        None
    }

    /// Primeira palavra do fabricante (`Intel Corporation` → `Intel`).
    fn short_vendor(&self, vendor: u16) -> Option<String> {
        self.vendor(vendor)?
            .split_whitespace()
            .next()
            .map(str::to_owned)
    }
}

fn pci_id(path: &Path) -> Option<u16> {
    u16::from_str_radix(read(path)?.trim_start_matches("0x"), 16).ok()
}

/// Dispositivo PCI do barramento 0 com a classe pedida (`0x0600` ponte de host,
/// `0x0601` ponte ISA/LPC): `(fabricante, dispositivo)`.
fn pci_bridge(sys: &Path, class: &str) -> Option<(u16, u16)> {
    let mut entries: Vec<_> = fs::read_dir(sys.join("bus/pci/devices"))
        .ok()?
        .flatten()
        .collect();
    entries.sort_by_key(|e| e.file_name());
    entries.into_iter().find_map(|entry| {
        let path = entry.path();
        read(&path.join("class"))?
            .starts_with(class)
            .then_some(())?;
        Some((pci_id(&path.join("vendor"))?, pci_id(&path.join("device"))?))
    })
}

/// Nome do código entre colchetes no nome da ponte de host (`[Coffee Lake S]`).
fn codename(name: &str) -> Option<String> {
    let start = name.rfind('[')?;
    let end = name[start..].find(']')? + start;
    Some(name[start + 1..end].trim().to_owned()).filter(|s| !s.is_empty())
}

/// Fabricante pelo prefixo do modelo, quando o barramento não informa (ATA).
fn disk_maker(model: &str) -> Option<&'static str> {
    const PREFIXES: &[(&str, &str)] = &[
        ("WDC", "Western Digital"),
        ("WDS", "Western Digital"),
        ("WD_", "Western Digital"),
        ("WD ", "Western Digital"),
        ("ST", "Seagate"),
        ("Samsung", "Samsung"),
        ("SAMSUNG", "Samsung"),
        ("XPG", "ADATA"),
        ("ADATA", "ADATA"),
        ("KINGSTON", "Kingston"),
        ("Crucial", "Crucial"),
        ("CT", "Crucial"),
        ("TOSHIBA", "Toshiba"),
        ("INTEL", "Intel"),
        ("SanDisk", "SanDisk"),
        ("HGST", "HGST"),
        ("Hitachi", "Hitachi"),
        ("Micron", "Micron"),
        ("Corsair", "Corsair"),
        ("Seagate", "Seagate"),
    ];
    PREFIXES
        .iter()
        .find(|(prefix, _)| model.starts_with(prefix))
        .map(|(_, maker)| *maker)
}

/// Número de bits de um formato do PulseAudio (`s24le` → 24).
fn sample_bits(format: &str) -> Option<String> {
    let float = format.starts_with("float");
    let digits: String = format.chars().filter(char::is_ascii_digit).collect();
    let bits: u32 = digits.parse().ok()?;
    Some(if float {
        format!("{bits} bits (float)")
    } else {
        format!("{bits} bits")
    })
}

/// Formato de saída por placa ALSA, a partir de `LC_ALL=C pactl list sinks`:
/// placa → (formato legível, canais).
pub fn parse_pactl_sinks(text: &str) -> BTreeMap<u32, (String, u32)> {
    let mut out = BTreeMap::new();
    for block in text.split("\nSink #") {
        let mut spec = None;
        let mut card = None;
        for line in block.lines().map(str::trim) {
            if let Some(value) = line.strip_prefix("Sample Specification:") {
                spec = Some(value.trim().to_owned());
            } else if let Some(value) = line.strip_prefix("alsa.card = ") {
                card = value.trim_matches('"').parse::<u32>().ok();
            }
        }
        let (Some(spec), Some(card)) = (spec, card) else {
            continue;
        };
        let parts: Vec<&str> = spec.split_whitespace().collect();
        let [format, channels, rate] = parts[..] else {
            continue;
        };
        let Some(channels) = channels.strip_suffix("ch").and_then(|c| c.parse().ok()) else {
            continue;
        };
        let rate = rate.strip_suffix("Hz").unwrap_or(rate);
        let text = match sample_bits(format) {
            Some(bits) => format!("{bits}, {rate} Hz"),
            None => format!("{format}, {rate} Hz"),
        };
        out.entry(card).or_insert((text, channels));
    }
    out
}

/// Lê o formato de saída de cada placa pelo `pactl`; vazio se não houver servidor.
pub fn pactl_sinks() -> BTreeMap<u32, (String, u32)> {
    std::process::Command::new("pactl")
        .args(["list", "sinks"])
        .env("LC_ALL", "C")
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| parse_pactl_sinks(&String::from_utf8_lossy(&out.stdout)))
        .unwrap_or_default()
}

/// Preenche Formato e Número de canais dos itens de áudio.
pub fn apply_audio(specs: &mut Specs, sinks: &BTreeMap<u32, (String, u32)>) {
    for (index, card) in specs.audio_cards.iter().enumerate() {
        let Some((format, channels)) = sinks.get(card) else {
            continue;
        };
        if let Some(fields) = specs.details.get_mut(&format!("audio:{index}")) {
            set_field(fields, "Formato", format.clone());
            set_field(fields, "Número de canais", channels.to_string());
        }
    }
}

/// Endereço exibido por interface: IPv4 primeiro, senão IPv6 global.
#[cfg(unix)]
pub fn interface_addresses() -> BTreeMap<String, String> {
    let mut v4: BTreeMap<String, String> = BTreeMap::new();
    let mut v6: BTreeMap<String, String> = BTreeMap::new();
    let Ok(addresses) = nix::ifaddrs::getifaddrs() else {
        return BTreeMap::new();
    };
    for entry in addresses {
        let Some(address) = entry.address else {
            continue;
        };
        if let Some(ip) = address.as_sockaddr_in() {
            v4.entry(entry.interface_name)
                .or_insert(ip.ip().to_string());
        } else if let Some(ip) = address.as_sockaddr_in6() {
            let ip = ip.ip();
            // Fora link-local (fe80::/10): esse endereço não identifica a máquina na rede.
            if (ip.segments()[0] & 0xffc0) != 0xfe80 && !ip.is_loopback() {
                v6.entry(entry.interface_name).or_insert(ip.to_string());
            }
        }
    }
    for (name, ip) in v6 {
        v4.entry(name).or_insert(ip);
    }
    v4
}

#[cfg(not(unix))]
pub fn interface_addresses() -> BTreeMap<String, String> {
    BTreeMap::new()
}

/// Atualiza o Endereço IP de cada adaptador; sem endereço, mostra indisponível.
pub fn apply_addresses(specs: &mut Specs, addresses: &BTreeMap<String, String>) {
    for (index, interface) in specs.network_interfaces.iter().enumerate() {
        if let Some(fields) = specs.details.get_mut(&format!("network:{index}")) {
            let value = addresses
                .get(interface)
                .cloned()
                .unwrap_or_else(|| UNAVAILABLE.into());
            set_field(fields, "Endereço IP", value);
        }
    }
}

fn set_field(fields: &mut [(String, String)], label: &str, value: String) {
    if let Some(field) = fields.iter_mut().find(|(name, _)| name == label) {
        field.1 = value;
    }
}

/// Módulos de RAM das propriedades `MEMORY_DEVICE_*` do udev: (nome, campos).
fn memory_modules(dmi: &BTreeMap<String, String>) -> Vec<(String, Vec<(String, String)>)> {
    let mut out = Vec::new();
    for index in 0.. {
        let key = |field: &str| format!("MEMORY_DEVICE_{index}_{field}");
        if !dmi
            .keys()
            .any(|k| k.starts_with(&format!("MEMORY_DEVICE_{index}_")))
        {
            break;
        }
        let Some(size) = dmi
            .get(&key("SIZE"))
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|&size| size > 0)
        else {
            continue; // Slot vazio.
        };
        let get = |field: &str| {
            dmi.get(&key(field))
                .map(|v| v.trim().to_owned())
                .filter(|v| !v.is_empty() && v != "Unknown" && v != "Not Specified")
        };
        let part = get("PART_NUMBER");
        let name = match (get("MANUFACTURER"), &part) {
            (Some(maker), Some(part)) => format!("{maker} {part}"),
            (None, Some(part)) => part.clone(),
            _ => get("LOCATOR").unwrap_or_else(|| format!("DIMM {index}")),
        };
        let speed = get("SPEED_MTS").or_else(|| get("CONFIGURED_SPEED_MTS"));
        out.push((
            name,
            vec![
                ("Modelo".into(), part.unwrap_or_else(|| UNAVAILABLE.into())),
                (
                    "Capacidade".into(),
                    format!("{:.0} GB", size as f64 / 1024f64.powi(3)),
                ),
                (
                    "Frequência padrão".into(),
                    speed.map_or(UNAVAILABLE.into(), |s| format!("{s} MT/s")),
                ),
                (
                    "Tipo".into(),
                    get("TYPE").unwrap_or_else(|| UNAVAILABLE.into()),
                ),
                ("Tempos".into(), UNAVAILABLE.into()),
            ],
        ));
    }
    out
}

fn read(path: &Path) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_owned())
}

fn khz_to_mhz(path: &Path) -> Option<f64> {
    read(path)?.parse::<f64>().ok().map(|v| v / 1000.0)
}

pub fn read_specs(sources: &Sources) -> Specs {
    let (sys, proc_) = (sources.sys.as_path(), sources.proc_.as_path());
    let udev = sources.run.join("udev/data");
    let pci = PciIds::load(&sources.pci_ids);
    let host = pci_bridge(sys, "0x0600");
    let lpc = pci_bridge(sys, "0x0601");
    let mut specs = Specs::default();
    let cpuinfo = fs::read_to_string(proc_.join("cpuinfo")).unwrap_or_default();
    let mut cores = std::collections::BTreeSet::new();
    for line in cpuinfo.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "model name" if specs.cpu.name.is_empty() => specs.cpu.name = value.to_owned(),
            "vendor_id" if specs.cpu.vendor.is_empty() => {
                specs.cpu.vendor = match value {
                    "GenuineIntel" => "Intel".into(),
                    "AuthenticAMD" => "AMD".into(),
                    other => other.to_owned(),
                }
            }
            "processor" => specs.cpu.threads += 1,
            "core id" => {
                cores.insert(value.to_owned());
            }
            _ => {}
        }
    }
    specs.cpu.cores = cores.len().max(usize::from(specs.cpu.threads > 0));
    let cpufreq = sys.join("devices/system/cpu/cpu0/cpufreq");
    specs.cpu.base_mhz = khz_to_mhz(&cpufreq.join("base_frequency"));
    specs.cpu.max_mhz = khz_to_mhz(&cpufreq.join("cpuinfo_max_freq"));
    let dmi = sys.join("class/dmi/id");
    specs.motherboard = match (
        read(&dmi.join("board_vendor")),
        read(&dmi.join("board_name")),
    ) {
        (Some(vendor), Some(name)) => Some(format!("{vendor} {name}")),
        (None, Some(name)) => Some(name),
        _ => None,
    };
    specs.memory_total_bytes = fs::read_to_string(proc_.join("meminfo"))
        .ok()
        .and_then(|text| {
            text.lines().find_map(|line| {
                line.strip_prefix("MemTotal:")?
                    .split_whitespace()
                    .next()?
                    .parse::<u64>()
                    .ok()
            })
        })
        .map(|kb| kb * 1024);
    let mounts = fs::read_to_string(proc_.join("mounts")).unwrap_or_default();
    if let Ok(entries) = fs::read_dir(sys.join("block")) {
        let mut names: Vec<_> = entries.flatten().collect();
        names.sort_by_key(|e| e.file_name());
        for entry in names {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with("loop") || name.starts_with("ram") || name.starts_with("zram") {
                continue;
            }
            let path = entry.path();
            let Some(sysfs_model) = read(&path.join("device/model")) else {
                continue;
            };
            let props = read(&path.join("dev"))
                .map(|dev| udev_properties(&udev.join(format!("b{dev}"))))
                .unwrap_or_default();
            // O modelo do sysfs para discos ATA vem truncado em 16 caracteres.
            let model = props
                .get("ID_MODEL_ENC")
                .map(|v| udev_unescape(v))
                .filter(|v| !v.is_empty())
                .unwrap_or(sysfs_model);
            let maker = disk_maker(&model)
                .map(str::to_owned)
                .or_else(|| {
                    // NVMe: fabricante do controlador PCI.
                    pci_id(&path.join("device/device/vendor"))
                        .and_then(|vendor| pci.short_vendor(vendor))
                })
                .or_else(|| {
                    props
                        .get("ID_VENDOR")
                        .filter(|v| v.as_str() != "ATA")
                        .cloned()
                })
                .unwrap_or_else(|| UNAVAILABLE.into());
            let mut formats = BTreeSet::new();
            let mut partitions: Vec<_> = fs::read_dir(&path)
                .map(|e| e.flatten().collect())
                .unwrap_or_default();
            partitions.sort_by_key(|e| e.file_name());
            for part in std::iter::once(path.clone()).chain(
                partitions
                    .iter()
                    .filter(|e| e.path().join("partition").exists())
                    .map(|e| e.path()),
            ) {
                let fs_type = read(&part.join("dev"))
                    .and_then(|dev| {
                        udev_properties(&udev.join(format!("b{dev}")))
                            .get("ID_FS_TYPE")
                            .cloned()
                    })
                    .or_else(|| {
                        let device = format!("/dev/{}", part.file_name()?.to_string_lossy());
                        mounts.lines().find_map(|line| {
                            let fields: Vec<_> = line.split_whitespace().collect();
                            (fields.len() > 2 && fields[0] == device).then(|| fields[2].to_owned())
                        })
                    });
                if let Some(fs_type) = fs_type.filter(|v| !v.is_empty()) {
                    formats.insert(fs_type);
                }
            }
            let capacity = read(&path.join("size"))
                .and_then(|v| v.parse::<u64>().ok())
                .map_or(UNAVAILABLE.into(), |sectors| {
                    format!("{:.0} GB", sectors as f64 * 512.0 / 1e9)
                });
            specs.details.insert(
                format!("storage:{}", specs.storage.len()),
                vec![
                    ("Nome".into(), model.clone()),
                    ("Fabricante".into(), maker),
                    (
                        "Formato".into(),
                        if formats.is_empty() {
                            UNAVAILABLE.into()
                        } else {
                            formats.into_iter().collect::<Vec<_>>().join(", ")
                        },
                    ),
                    ("Capacidade".into(), capacity),
                ],
            );
            specs.storage.push(model);
        }
    }
    if let Ok(text) = fs::read_to_string(proc_.join("asound/cards")) {
        // Formato: " 0 [PCH            ]: HDA-Intel - HDA Intel PCH"
        for line in text.lines().filter(|l| l.contains("]:")) {
            let card = line.split_whitespace().next().and_then(|n| n.parse().ok());
            if let (Some(card), Some((_, description))) = (card, line.split_once(" - ")) {
                specs.audio.push(description.trim().to_owned());
                specs.audio_cards.push(card);
            }
        }
    }
    if let Ok(entries) = fs::read_dir(sys.join("class/net")) {
        let mut names: Vec<_> = entries.flatten().collect();
        names.sort_by_key(|e| e.file_name());
        for entry in names {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name == "lo" || !entry.path().join("device").exists() {
                continue;
            }
            let kind = if entry.path().join("wireless").exists() {
                "Wi-Fi"
            } else {
                "Ethernet"
            };
            specs.network.push(format!("{kind} ({name})"));
            specs.network_interfaces.push(name);
        }
    }
    if !specs.cpu.name.is_empty() {
        specs.details.insert(
            "cpu".into(),
            vec![
                ("Fabricante".into(), specs.cpu.vendor.clone()),
                (
                    "Nome do código".into(),
                    host.and_then(|(v, d)| codename(pci.device(v, d)?))
                        .unwrap_or_else(|| UNAVAILABLE.into()),
                ),
                (
                    "Velocidade padrão do clock".into(),
                    specs
                        .cpu
                        .base_mhz
                        .map_or("—".into(), |v| format!("{v:.0} MHz")),
                ),
                ("Tipo de soquete".into(), "—".into()),
                ("TDP".into(), "—".into()),
            ],
        );
    }
    if specs.motherboard.is_some() {
        specs.details.insert(
            "board".into(),
            vec![
                (
                    "Modelo".into(),
                    read(&dmi.join("board_name")).unwrap_or_else(|| "—".into()),
                ),
                // Convenção do CPU-Z: chipset pela ponte de host, southbridge pelo PCH/LPC.
                (
                    "Chipset".into(),
                    host.and_then(|(v, d)| {
                        Some(format!(
                            "{} {}",
                            pci.short_vendor(v)?,
                            codename(pci.device(v, d)?)?
                        ))
                    })
                    .unwrap_or_else(|| UNAVAILABLE.into()),
                ),
                (
                    "Southbridge".into(),
                    lpc.and_then(|(v, d)| {
                        let name = pci.device(v, d)?;
                        let short = name
                            .split(" Chipset")
                            .next()
                            .and_then(|n| n.split(" LPC").next())
                            .unwrap_or(name)
                            .trim();
                        Some(format!("{} {short}", pci.short_vendor(v)?))
                    })
                    .unwrap_or_else(|| UNAVAILABLE.into()),
                ),
                (
                    "Versão do BIOS".into(),
                    read(&dmi.join("bios_version")).unwrap_or_else(|| "—".into()),
                ),
                (
                    "Data do BIOS".into(),
                    read(&dmi.join("bios_date")).unwrap_or_else(|| "—".into()),
                ),
            ],
        );
    }
    let modules = memory_modules(&udev_properties(&udev.join("+dmi:id")));
    for (index, (name, fields)) in modules.into_iter().enumerate() {
        specs.memory_modules.push(name);
        specs.details.insert(format!("ram:{index}"), fields);
    }
    if specs.memory_modules.is_empty()
        && let Some(bytes) = specs.memory_total_bytes
    {
        specs.memory_modules.push("Memória do sistema".into());
        specs.details.insert(
            "ram:0".into(),
            vec![
                ("Modelo".into(), "—".into()),
                (
                    "Capacidade".into(),
                    format!("{:.1} GB", bytes as f64 / 1024f64.powi(3)),
                ),
                ("Frequência padrão".into(), "—".into()),
                ("Tipo".into(), "—".into()),
                ("Tempos".into(), "—".into()),
            ],
        );
    }
    for (index, name) in specs.audio.iter().enumerate() {
        specs.details.insert(
            format!("audio:{index}"),
            vec![
                ("Nome".into(), name.clone()),
                ("Formato".into(), "—".into()),
                ("Número de canais".into(), "—".into()),
            ],
        );
    }
    for (index, name) in specs.network.iter().enumerate() {
        specs.details.insert(
            format!("network:{index}"),
            vec![
                ("Nome do adaptador".into(), name.clone()),
                (
                    "Tipo de conexão".into(),
                    if name.starts_with("Wi-Fi") {
                        "Wireless"
                    } else {
                        "Ethernet"
                    }
                    .into(),
                ),
                ("Endereço IP".into(), "—".into()),
            ],
        );
    }
    specs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sensors::tests::write;

    fn sources(root: &Path) -> Sources {
        Sources {
            sys: root.join("sys"),
            proc_: root.join("proc"),
            run: root.join("run"),
            pci_ids: root.join("pci.ids"),
        }
    }

    fn field<'a>(specs: &'a Specs, key: &str, label: &str) -> &'a str {
        &specs.details[key]
            .iter()
            .find(|(name, _)| name == label)
            .unwrap_or_else(|| panic!("{key} sem {label}"))
            .1
    }

    #[test]
    fn reads_hardware_details_from_udev_and_pci_ids() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(
            root,
            "proc/cpuinfo",
            "processor\t: 0\nvendor_id\t: GenuineIntel\nmodel name\t: i9\n",
        );
        write(root, "sys/class/dmi/id/board_name", "ROG MAXIMUS XI HERO\n");
        write(
            root,
            "pci.ids",
            "# comentário\n8086  Intel Corporation\n\t3e30  8th/9th Gen Core 8-core Desktop Processor Host Bridge/DRAM Registers [Coffee Lake S]\n\ta305  Z390 Chipset LPC/eSPI Controller\n15b7  Sandisk Corp\n\t5006  WD Black SN750\n",
        );
        for (slot, class, device) in [
            ("0000:00:00.0", "0x060000", "0x3e30"),
            ("0000:00:1f.0", "0x060100", "0xa305"),
        ] {
            write(root, &format!("sys/bus/pci/devices/{slot}/class"), class);
            write(
                root,
                &format!("sys/bus/pci/devices/{slot}/vendor"),
                "0x8086",
            );
            write(root, &format!("sys/bus/pci/devices/{slot}/device"), device);
        }
        write(
            root,
            "run/udev/data/+dmi:id",
            "E:MEMORY_DEVICE_0_SIZE=8589934592\nE:MEMORY_DEVICE_0_TYPE=DDR4\nE:MEMORY_DEVICE_0_SPEED_MTS=2133\nE:MEMORY_DEVICE_0_MANUFACTURER=G-Skill\nE:MEMORY_DEVICE_0_PART_NUMBER=F4-3200C16-8GTZR\nE:MEMORY_DEVICE_1_LOCATOR=ChannelA-DIMM2\nE:MEMORY_DEVICE_2_SIZE=8589934592\nE:MEMORY_DEVICE_2_LOCATOR=ChannelB-DIMM1\n",
        );
        // Disco ATA: modelo truncado no sysfs, completo no udev; partições com formatos.
        write(root, "sys/block/sdb/device/model", "Samsung SSD 850\n");
        write(root, "sys/block/sdb/device/vendor", "ATA\n");
        write(root, "sys/block/sdb/dev", "8:16\n");
        write(root, "sys/block/sdb/size", "488397168\n");
        write(root, "sys/block/sdb/sdb1/partition", "1\n");
        write(root, "sys/block/sdb/sdb1/dev", "8:17\n");
        write(root, "sys/block/sdb/sdb2/partition", "2\n");
        write(root, "sys/block/sdb/sdb2/dev", "8:18\n");
        write(
            root,
            "run/udev/data/b8:16",
            "E:ID_MODEL=Samsung_SSD_850_EVO_250GB\nE:ID_MODEL_ENC=Samsung\\x20SSD\\x20850\\x20EVO\\x20250GB\\x20\\x20\n",
        );
        write(root, "run/udev/data/b8:17", "E:ID_FS_TYPE=vfat\n");
        write(root, "run/udev/data/b8:18", "E:ID_FS_TYPE=ext4\n");
        // NVMe sem prefixo conhecido: fabricante pelo controlador PCI.
        write(root, "sys/block/nvme0n1/device/model", "Black SN750\n");
        write(root, "sys/block/nvme0n1/device/device/vendor", "0x15b7\n");
        let specs = read_specs(&sources(root));
        assert_eq!(field(&specs, "cpu", "Nome do código"), "Coffee Lake S");
        assert_eq!(field(&specs, "board", "Chipset"), "Intel Coffee Lake S");
        assert_eq!(field(&specs, "board", "Southbridge"), "Intel Z390");
        assert_eq!(
            specs.memory_modules,
            vec!["G-Skill F4-3200C16-8GTZR", "ChannelB-DIMM1"]
        );
        assert_eq!(field(&specs, "ram:0", "Capacidade"), "8 GB");
        assert_eq!(field(&specs, "ram:0", "Frequência padrão"), "2133 MT/s");
        assert_eq!(field(&specs, "ram:0", "Tipo"), "DDR4");
        assert_eq!(field(&specs, "ram:1", "Modelo"), "—");
        assert_eq!(
            specs.storage,
            vec!["Black SN750", "Samsung SSD 850 EVO 250GB"]
        );
        assert_eq!(field(&specs, "storage:0", "Fabricante"), "Sandisk");
        assert_eq!(field(&specs, "storage:1", "Fabricante"), "Samsung");
        assert_eq!(field(&specs, "storage:1", "Formato"), "ext4, vfat");
        assert_eq!(field(&specs, "storage:1", "Capacidade"), "250 GB");
    }

    #[test]
    fn audio_format_comes_from_pactl_by_alsa_card() {
        let text = "Sink #59\n\tSample Specification: float32le 2ch 48000Hz\n\tProperties:\n\t\tmedia.class = \"Audio/Sink\"\n\nSink #211\n\tSample Specification: s24le 2ch 48000Hz\n\tProperties:\n\t\talsa.card = \"1\"\n\nSink #214\n\tSample Specification: s32le 6ch 44100Hz\n\tProperties:\n\t\talsa.card = \"0\"\n";
        let sinks = parse_pactl_sinks(text);
        assert_eq!(sinks.len(), 2);
        assert_eq!(sinks[&1], ("24 bits, 48000 Hz".to_owned(), 2));
        let mut specs = Specs {
            audio: vec!["HDA Intel PCH".into(), "Arctis".into(), "HDA NVidia".into()],
            audio_cards: vec![0, 1, 2],
            ..Default::default()
        };
        for index in 0..3 {
            specs.details.insert(
                format!("audio:{index}"),
                vec![
                    ("Nome".into(), "x".into()),
                    ("Formato".into(), "—".into()),
                    ("Número de canais".into(), "—".into()),
                ],
            );
        }
        apply_audio(&mut specs, &sinks);
        assert_eq!(field(&specs, "audio:0", "Formato"), "32 bits, 44100 Hz");
        assert_eq!(field(&specs, "audio:0", "Número de canais"), "6");
        assert_eq!(field(&specs, "audio:1", "Formato"), "24 bits, 48000 Hz");
        // Placa sem saída no servidor de som continua indisponível.
        assert_eq!(field(&specs, "audio:2", "Formato"), "—");
    }

    #[test]
    fn network_address_updates_and_clears() {
        let mut specs = Specs {
            network: vec!["Wi-Fi (wlp2s0)".into()],
            network_interfaces: vec!["wlp2s0".into()],
            ..Default::default()
        };
        specs
            .details
            .insert("network:0".into(), vec![("Endereço IP".into(), "—".into())]);
        let mut addresses = BTreeMap::from([("wlp2s0".to_owned(), "192.168.0.186".to_owned())]);
        apply_addresses(&mut specs, &addresses);
        assert_eq!(field(&specs, "network:0", "Endereço IP"), "192.168.0.186");
        addresses.clear();
        apply_addresses(&mut specs, &addresses);
        assert_eq!(field(&specs, "network:0", "Endereço IP"), "—");
    }

    #[test]
    fn reads_synthetic_system() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(
            root,
            "proc/cpuinfo",
            "processor\t: 0\nvendor_id\t: GenuineIntel\nmodel name\t: Intel(R) Core(TM) i9-9900K CPU @ 3.60GHz\ncore id\t: 0\n\nprocessor\t: 1\nvendor_id\t: GenuineIntel\ncore id\t: 0\n\nprocessor\t: 2\ncore id\t: 1\n",
        );
        write(
            root,
            "sys/devices/system/cpu/cpu0/cpufreq/base_frequency",
            "3600000\n",
        );
        write(
            root,
            "sys/devices/system/cpu/cpu0/cpufreq/cpuinfo_max_freq",
            "5000000\n",
        );
        write(
            root,
            "sys/class/dmi/id/board_vendor",
            "ASUSTeK COMPUTER INC.\n",
        );
        write(root, "sys/class/dmi/id/board_name", "ROG MAXIMUS XI HERO\n");
        write(root, "proc/meminfo", "MemTotal:       32768000 kB\n");
        write(root, "sys/block/nvme0n1/device/model", "XPG GAMMIX S41\n");
        write(root, "sys/block/loop0/device/model", "x\n");
        write(
            root,
            "proc/asound/cards",
            " 0 [PCH            ]: HDA-Intel - HDA Intel PCH\n                      HDA Intel PCH at 0xa1234\n 1 [AE5            ]: HDA-Intel - Sound BlasterX AE-5 Plus\n",
        );
        write(root, "sys/class/net/wlp5s0/device/uevent", "");
        write(root, "sys/class/net/wlp5s0/wireless/x", "");
        write(root, "sys/class/net/enp4s0/device/uevent", "");
        write(root, "sys/class/net/lo/address", "");
        let specs = read_specs(&sources(root));
        assert_eq!(specs.cpu.vendor, "Intel");
        assert_eq!((specs.cpu.cores, specs.cpu.threads), (2, 3));
        assert_eq!(specs.cpu.base_mhz, Some(3600.0));
        assert_eq!(specs.cpu.max_mhz, Some(5000.0));
        assert_eq!(
            specs.motherboard.as_deref(),
            Some("ASUSTeK COMPUTER INC. ROG MAXIMUS XI HERO")
        );
        assert_eq!(specs.memory_total_bytes, Some(32768000 * 1024));
        assert_eq!(specs.storage, vec!["XPG GAMMIX S41"]);
        assert_eq!(
            specs.audio,
            vec!["HDA Intel PCH", "Sound BlasterX AE-5 Plus"]
        );
        assert_eq!(specs.network, vec!["Ethernet (enp4s0)", "Wi-Fi (wlp5s0)"]);
        assert_eq!(specs.network_interfaces, vec!["enp4s0", "wlp5s0"]);
        assert_eq!(specs.audio_cards, vec![0, 1]);
        // Sem dados do udev, a RAM aparece como total.
        assert_eq!(specs.memory_modules, vec!["Memória do sistema"]);
    }

    #[test]
    fn empty_system_yields_empty_specs() {
        let dir = tempfile::tempdir().unwrap();
        let specs = read_specs(&sources(dir.path()));
        assert_eq!(specs, Specs::default());
    }
}
