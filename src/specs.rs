//! Especificações do sistema (tela "Especificações do sistema" do CAM), lidas uma
//! vez de sysfs e `/proc`, sem privilégios. Módulos de RAM individuais exigem
//! `dmidecode` como root e por isso aparecem como total.
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

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
}

fn read(path: &Path) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_owned())
}

fn khz_to_mhz(path: &Path) -> Option<f64> {
    read(path)?.parse::<f64>().ok().map(|v| v / 1000.0)
}

pub fn read_specs(sys: &Path, proc_: &Path) -> Specs {
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
    if let Ok(entries) = fs::read_dir(sys.join("block")) {
        let mut names: Vec<_> = entries.flatten().collect();
        names.sort_by_key(|e| e.file_name());
        for entry in names {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with("loop") || name.starts_with("ram") || name.starts_with("zram") {
                continue;
            }
            if let Some(model) = read(&entry.path().join("device/model")) {
                specs.storage.push(model);
            }
        }
    }
    if let Ok(text) = fs::read_to_string(proc_.join("asound/cards")) {
        // Formato: " 0 [PCH            ]: HDA-Intel - HDA Intel PCH"
        for line in text.lines().filter(|l| l.contains("]:")) {
            if let Some((_, description)) = line.split_once(" - ") {
                specs.audio.push(description.trim().to_owned());
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
        }
    }
    specs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sensors::tests::write;

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
        let specs = read_specs(&root.join("sys"), &root.join("proc"));
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
    }

    #[test]
    fn empty_system_yields_empty_specs() {
        let dir = tempfile::tempdir().unwrap();
        let specs = read_specs(&dir.path().join("sys"), &dir.path().join("proc"));
        assert_eq!(specs, Specs::default());
    }
}
