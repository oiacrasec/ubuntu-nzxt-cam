use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Cpu {
    pub name: String,
    /// °C do sensor de pacote (Intel `Package id 0`, AMD `Tctl`).
    pub package_temperature: Option<f64>,
    /// °C, média dos núcleos (Intel `Core N`, AMD `Tccd N`).
    pub average_temperature: Option<f64>,
    pub load_percent: Option<f64>,
    pub clock_mhz: Option<f64>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Gpu {
    pub name: String,
    pub temperature: Option<f64>,
    pub load_percent: Option<f64>,
    pub clock_mhz: Option<f64>,
    pub fan_percent: Option<f64>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Memory {
    pub total_bytes: u64,
    pub used_bytes: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Storage {
    pub name: String,
    pub temperature: Option<f64>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Network {
    pub receive_bytes_per_second: Option<f64>,
    pub transmit_bytes_per_second: Option<f64>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Readings {
    pub cpu: Cpu,
    pub gpus: Vec<Gpu>,
    pub memory: Option<Memory>,
    pub storage: Vec<Storage>,
    pub network: Network,
    /// Falhas de leitura por fonte; ausência de fonte não é erro.
    pub errors: BTreeMap<String, String>,
}

impl Readings {
    pub fn cpu_temperature(&self, average: bool) -> Option<f64> {
        if average {
            self.cpu
                .average_temperature
                .or(self.cpu.package_temperature)
        } else {
            self.cpu
                .package_temperature
                .or(self.cpu.average_temperature)
        }
    }

    pub fn gpu_temperature(&self, index: usize) -> Option<f64> {
        self.gpus.get(index).and_then(|gpu| gpu.temperature)
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct CpuTimes {
    busy: u64,
    total: u64,
}

pub struct Sensors {
    sys: PathBuf,
    proc_: PathBuf,
    previous_cpu: Option<CpuTimes>,
    previous_net: Option<(Instant, u64, u64)>,
    #[cfg(feature = "nvidia")]
    nvml: Option<std::result::Result<nvml_wrapper::Nvml, String>>,
}

impl Default for Sensors {
    fn default() -> Self {
        Self::new("/sys", "/proc")
    }
}

fn read_trimmed(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok().map(|s| s.trim().to_owned())
}

/// Valor hwmon em milésimos (`temp*_input`), convertido para °C.
fn read_millis(path: &Path) -> Option<f64> {
    let value = read_trimmed(path)?.parse::<f64>().ok()? / 1000.0;
    (value.is_finite() && (-60.0..=200.0).contains(&value)).then_some(value)
}

fn average(values: &[f64]) -> Option<f64> {
    (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
}

struct Hwmon {
    name: String,
    dir: PathBuf,
}

impl Hwmon {
    /// Pares (rótulo, °C) de `tempN_label`/`tempN_input`.
    fn temperatures(&self) -> Vec<(String, f64)> {
        let mut out = Vec::new();
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return out;
        };
        let mut inputs: Vec<_> = entries
            .flatten()
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| n.starts_with("temp") && n.ends_with("_input"))
            .collect();
        inputs.sort();
        for input in inputs {
            let stem = input.trim_end_matches("_input");
            let label = read_trimmed(&self.dir.join(format!("{stem}_label")))
                .unwrap_or_else(|| stem.to_owned());
            if let Some(value) = read_millis(&self.dir.join(&input)) {
                out.push((label, value));
            }
        }
        out
    }
}

impl Sensors {
    pub fn new(sys: impl Into<PathBuf>, proc_: impl Into<PathBuf>) -> Self {
        Self {
            sys: sys.into(),
            proc_: proc_.into(),
            previous_cpu: None,
            previous_net: None,
            #[cfg(feature = "nvidia")]
            nvml: None,
        }
    }

    fn hwmons(&self) -> Vec<Hwmon> {
        let mut out = Vec::new();
        let Ok(entries) = fs::read_dir(self.sys.join("class/hwmon")) else {
            return out;
        };
        for entry in entries.flatten() {
            let dir = entry.path();
            if let Some(name) = read_trimmed(&dir.join("name")) {
                out.push(Hwmon { name, dir });
            }
        }
        out.sort_by(|a, b| a.dir.cmp(&b.dir));
        out
    }

    pub fn read(&mut self) -> Readings {
        let mut readings = Readings::default();
        let hwmons = self.hwmons();
        self.read_cpu(&hwmons, &mut readings);
        self.read_memory(&mut readings);
        self.read_network(&mut readings);
        for hwmon in &hwmons {
            if matches!(hwmon.name.as_str(), "nvme" | "drivetemp") {
                let temperature = hwmon
                    .temperatures()
                    .into_iter()
                    .find(|(label, _)| label == "Composite" || label == "temp1")
                    .map(|(_, value)| value);
                let model = read_trimmed(&hwmon.dir.join("device/model"));
                readings.storage.push(Storage {
                    name: model.unwrap_or_else(|| hwmon.name.clone()),
                    temperature,
                });
            }
            if hwmon.name == "amdgpu" {
                let device = hwmon.dir.join("device");
                readings.gpus.push(Gpu {
                    name: "AMD Radeon".into(),
                    temperature: hwmon
                        .temperatures()
                        .into_iter()
                        .find(|(label, _)| label == "edge")
                        .map(|(_, value)| value),
                    load_percent: read_trimmed(&device.join("gpu_busy_percent"))
                        .and_then(|v| v.parse().ok()),
                    clock_mhz: None,
                    fan_percent: None,
                });
            }
        }
        self.read_nvidia(&mut readings);
        readings
    }

    fn read_cpu(&mut self, hwmons: &[Hwmon], readings: &mut Readings) {
        let cpuinfo = fs::read_to_string(self.proc_.join("cpuinfo")).unwrap_or_default();
        let mut mhz = Vec::new();
        for line in cpuinfo.lines() {
            if let Some((key, value)) = line.split_once(':') {
                match key.trim() {
                    "model name" if readings.cpu.name.is_empty() => {
                        readings.cpu.name = value.trim().to_owned()
                    }
                    "cpu MHz" => mhz.extend(value.trim().parse::<f64>().ok()),
                    _ => {}
                }
            }
        }
        readings.cpu.clock_mhz = average(&mhz);
        for hwmon in hwmons {
            let temperatures = hwmon.temperatures();
            match hwmon.name.as_str() {
                "coretemp" => {
                    let cores: Vec<f64> = temperatures
                        .iter()
                        .filter(|(label, _)| label.starts_with("Core "))
                        .map(|(_, v)| *v)
                        .collect();
                    readings.cpu.package_temperature = temperatures
                        .iter()
                        .find(|(label, _)| label.starts_with("Package id"))
                        .map(|(_, v)| *v);
                    readings.cpu.average_temperature = average(&cores);
                }
                "k10temp" | "zenpower" => {
                    let ccds: Vec<f64> = temperatures
                        .iter()
                        .filter(|(label, _)| label.starts_with("Tccd"))
                        .map(|(_, v)| *v)
                        .collect();
                    readings.cpu.package_temperature = temperatures
                        .iter()
                        .find(|(label, _)| label == "Tctl" || label == "Tdie")
                        .map(|(_, v)| *v);
                    readings.cpu.average_temperature = average(&ccds);
                }
                _ => {}
            }
        }
        match fs::read_to_string(self.proc_.join("stat")) {
            Ok(stat) => {
                if let Some(times) = parse_cpu_times(&stat) {
                    if let Some(previous) = self.previous_cpu {
                        let total = times.total.saturating_sub(previous.total);
                        let busy = times.busy.saturating_sub(previous.busy);
                        if total > 0 {
                            readings.cpu.load_percent = Some(busy as f64 * 100.0 / total as f64);
                        }
                    }
                    self.previous_cpu = Some(times);
                }
            }
            Err(e) => {
                readings.errors.insert("cpu".into(), e.to_string());
            }
        }
    }

    fn read_memory(&self, readings: &mut Readings) {
        let Ok(text) = fs::read_to_string(self.proc_.join("meminfo")) else {
            return;
        };
        let field = |name: &str| {
            text.lines().find_map(|line| {
                let rest = line.strip_prefix(name)?.strip_prefix(':')?;
                rest.split_whitespace().next()?.parse::<u64>().ok()
            })
        };
        if let (Some(total), Some(available)) = (field("MemTotal"), field("MemAvailable")) {
            readings.memory = Some(Memory {
                total_bytes: total * 1024,
                used_bytes: total.saturating_sub(available) * 1024,
            });
        }
    }

    fn read_network(&mut self, readings: &mut Readings) {
        let Ok(text) = fs::read_to_string(self.proc_.join("net/dev")) else {
            return;
        };
        let (mut rx, mut tx) = (0u64, 0u64);
        for line in text.lines().skip(2) {
            let Some((name, values)) = line.split_once(':') else {
                continue;
            };
            if name.trim() == "lo" {
                continue;
            }
            let fields: Vec<u64> = values
                .split_whitespace()
                .filter_map(|v| v.parse().ok())
                .collect();
            if fields.len() >= 9 {
                rx += fields[0];
                tx += fields[8];
            }
        }
        let now = Instant::now();
        if let Some((then, prx, ptx)) = self.previous_net {
            let seconds = now.duration_since(then).as_secs_f64();
            if seconds > 0.0 {
                readings.network = Network {
                    receive_bytes_per_second: Some(rx.saturating_sub(prx) as f64 / seconds),
                    transmit_bytes_per_second: Some(tx.saturating_sub(ptx) as f64 / seconds),
                };
            }
        }
        self.previous_net = Some((now, rx, tx));
    }

    #[cfg(feature = "nvidia")]
    fn read_nvidia(&mut self, readings: &mut Readings) {
        use nvml_wrapper::enum_wrappers::device::{Clock, TemperatureSensor};
        let nvml = self
            .nvml
            .get_or_insert_with(|| nvml_wrapper::Nvml::init().map_err(|e| e.to_string()));
        let nvml = match nvml {
            Ok(nvml) => nvml,
            Err(_) => return, // Sem driver NVIDIA: fonte ausente, não é erro.
        };
        let count = match nvml.device_count() {
            Ok(count) => count,
            Err(e) => {
                readings.errors.insert("gpu".into(), e.to_string());
                return;
            }
        };
        for index in 0..count {
            let Ok(device) = nvml.device_by_index(index) else {
                continue;
            };
            readings.gpus.push(Gpu {
                name: device.name().unwrap_or_else(|_| "NVIDIA".into()),
                temperature: device
                    .temperature(TemperatureSensor::Gpu)
                    .ok()
                    .map(f64::from),
                load_percent: device.utilization_rates().ok().map(|u| f64::from(u.gpu)),
                clock_mhz: device.clock_info(Clock::Graphics).ok().map(f64::from),
                fan_percent: device.fan_speed(0).ok().map(f64::from),
            });
        }
    }

    #[cfg(not(feature = "nvidia"))]
    fn read_nvidia(&mut self, _: &mut Readings) {}
}

fn parse_cpu_times(stat: &str) -> Option<CpuTimes> {
    let line = stat.lines().find(|l| l.starts_with("cpu "))?;
    let values: Vec<u64> = line
        .split_whitespace()
        .skip(1)
        .filter_map(|v| v.parse().ok())
        .collect();
    if values.len() < 4 {
        return None;
    }
    // user nice system idle iowait irq softirq steal
    let idle = values[3] + values.get(4).copied().unwrap_or(0);
    let total: u64 = values.iter().take(8).sum();
    Some(CpuTimes {
        busy: total.saturating_sub(idle),
        total,
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn write(root: &Path, relative: &str, value: &str) {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, value).unwrap();
    }

    pub fn intel_tree(root: &Path, package: &str) {
        write(root, "sys/class/hwmon/hwmon2/name", "coretemp\n");
        write(root, "sys/class/hwmon/hwmon2/temp1_label", "Package id 0\n");
        write(root, "sys/class/hwmon/hwmon2/temp1_input", package);
        write(root, "sys/class/hwmon/hwmon2/temp2_label", "Core 0\n");
        write(root, "sys/class/hwmon/hwmon2/temp2_input", "40000\n");
        write(root, "sys/class/hwmon/hwmon2/temp3_label", "Core 1\n");
        write(root, "sys/class/hwmon/hwmon2/temp3_input", "44000\n");
        write(
            root,
            "proc/cpuinfo",
            "model name\t: Intel(R) Core(TM) i9-9900K CPU @ 3.60GHz\ncpu MHz\t\t: 3600.000\ncpu MHz\t\t: 4800.000\n",
        );
        write(root, "proc/stat", "cpu  100 0 100 800 0 0 0 0 0 0\n");
        write(
            root,
            "proc/meminfo",
            "MemTotal:       32768000 kB\nMemFree:  1 kB\nMemAvailable:   16384000 kB\n",
        );
        write(
            root,
            "proc/net/dev",
            "Inter-|\n face |\n    lo: 999 0 0 0 0 0 0 0 999 0 0 0 0 0 0 0\n  eth0: 1000 0 0 0 0 0 0 0 2000 0 0 0 0 0 0 0\n",
        );
    }

    fn sensors(root: &Path) -> Sensors {
        Sensors::new(root.join("sys"), root.join("proc"))
    }

    #[test]
    fn reads_intel_package_and_core_average() {
        let dir = tempfile::tempdir().unwrap();
        intel_tree(dir.path(), "47500\n");
        let readings = sensors(dir.path()).read();
        assert_eq!(readings.cpu.package_temperature, Some(47.5));
        assert_eq!(readings.cpu.average_temperature, Some(42.0));
        assert_eq!(readings.cpu_temperature(false), Some(47.5));
        assert_eq!(readings.cpu_temperature(true), Some(42.0));
        assert_eq!(readings.cpu.clock_mhz, Some(4200.0));
        assert!(readings.cpu.name.contains("i9-9900K"));
        let memory = readings.memory.unwrap();
        assert_eq!(memory.total_bytes, 32768000 * 1024);
        assert_eq!(memory.used_bytes, 16384000 * 1024);
    }

    #[test]
    fn cpu_load_needs_two_samples() {
        let dir = tempfile::tempdir().unwrap();
        intel_tree(dir.path(), "47500\n");
        let mut sensors = sensors(dir.path());
        assert_eq!(sensors.read().cpu.load_percent, None);
        write(dir.path(), "proc/stat", "cpu  150 0 150 900 0 0 0 0 0 0\n");
        let load = sensors.read().cpu.load_percent.unwrap();
        assert!((load - 50.0).abs() < 1e-9);
    }

    #[test]
    fn network_excludes_loopback() {
        let dir = tempfile::tempdir().unwrap();
        intel_tree(dir.path(), "47500\n");
        let mut sensors = sensors(dir.path());
        sensors.read();
        std::thread::sleep(std::time::Duration::from_millis(20));
        write(
            dir.path(),
            "proc/net/dev",
            "a\nb\n    lo: 99999 0 0 0 0 0 0 0 99999 0 0 0 0 0 0 0\n  eth0: 3000 0 0 0 0 0 0 0 2000 0 0 0 0 0 0 0\n",
        );
        let network = sensors.read().network;
        assert!(network.receive_bytes_per_second.unwrap() > 0.0);
        assert_eq!(network.transmit_bytes_per_second, Some(0.0));
    }

    #[test]
    fn rejects_implausible_and_missing_temperatures() {
        let dir = tempfile::tempdir().unwrap();
        intel_tree(dir.path(), "999999\n");
        let readings = sensors(dir.path()).read();
        assert_eq!(readings.cpu.package_temperature, None);
        assert_eq!(readings.cpu_temperature(false), Some(42.0));
        let empty = tempfile::tempdir().unwrap();
        let readings = sensors(empty.path()).read();
        assert_eq!(readings.cpu_temperature(false), None);
        assert!(readings.memory.is_none());
        assert!(readings.errors.contains_key("cpu"));
    }

    #[test]
    fn reads_nvme_and_amd_sources() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, "sys/class/hwmon/hwmon0/name", "nvme\n");
        write(root, "sys/class/hwmon/hwmon0/temp1_label", "Composite\n");
        write(root, "sys/class/hwmon/hwmon0/temp1_input", "38850\n");
        write(root, "sys/class/hwmon/hwmon1/name", "k10temp\n");
        write(root, "sys/class/hwmon/hwmon1/temp1_label", "Tctl\n");
        write(root, "sys/class/hwmon/hwmon1/temp1_input", "61000\n");
        write(root, "sys/class/hwmon/hwmon1/temp3_label", "Tccd1\n");
        write(root, "sys/class/hwmon/hwmon1/temp3_input", "55000\n");
        let readings = sensors(root).read();
        assert_eq!(readings.storage.len(), 1);
        assert_eq!(readings.storage[0].temperature, Some(38.85));
        assert_eq!(readings.cpu.package_temperature, Some(61.0));
        assert_eq!(readings.cpu.average_temperature, Some(55.0));
    }
}
