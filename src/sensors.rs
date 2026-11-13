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
pub struct Disk {
    pub mount: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Process {
    pub name: String,
    /// Fração de toda a CPU da máquina, como no CAM.
    pub cpu_percent: f64,
    pub ram_bytes: u64,
    #[serde(default)]
    pub gpu_percent: Option<f64>,
    #[serde(default)]
    pub download_bytes_per_second: Option<f64>,
    #[serde(default)]
    pub upload_bytes_per_second: Option<f64>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Readings {
    pub cpu: Cpu,
    pub gpus: Vec<Gpu>,
    pub memory: Option<Memory>,
    pub storage: Vec<Storage>,
    pub network: Network,
    pub disks: Vec<Disk>,
    pub processes: Vec<Process>,
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
    previous_processes: BTreeMap<u32, u64>,
    previous_process_total: Option<u64>,
    #[cfg(feature = "nvidia")]
    nvml: Option<std::result::Result<nvml_wrapper::Nvml, String>>,
}

/// Tamanho de página do Linux x86-64, usado para converter o RSS de `/proc/*/stat`.
const PAGE_SIZE: u64 = 4096;
const TOP_PROCESSES: usize = 5;

/// Nome, ticks de CPU (utime + stime) e RSS em páginas de `/proc/<pid>/stat`.
fn parse_process_stat(stat: &str) -> Option<(String, u64, u64)> {
    let open = stat.find('(')?;
    let close = stat.rfind(')')?;
    let name = stat.get(open + 1..close)?.to_owned();
    let fields: Vec<&str> = stat.get(close + 1..)?.split_whitespace().collect();
    // Após o nome: estado é o campo 3; utime 14, stime 15, rss 24 (numeração do proc(5)).
    let utime: u64 = fields.get(11)?.parse().ok()?;
    let stime: u64 = fields.get(12)?.parse().ok()?;
    let rss: u64 = fields.get(21)?.parse().ok()?;
    Some((name, utime + stime, rss))
}

/// Pontos de montagem de dispositivos reais em `/proc/mounts`, sem duplicar dispositivo.
pub fn real_mounts(text: &str) -> Vec<String> {
    let mut seen = Vec::new();
    let mut out = Vec::new();
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        let (Some(device), Some(mount), Some(kind)) = (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        if !device.starts_with("/dev/")
            || device.starts_with("/dev/loop")
            || matches!(kind, "squashfs" | "iso9660" | "udf")
            || seen.contains(&device)
        {
            continue;
        }
        seen.push(device);
        out.push(mount.replace("\\040", " "));
    }
    out
}

#[cfg(unix)]
fn disk_usage(mount: &str) -> Option<Disk> {
    let stat = nix::sys::statvfs::statvfs(mount).ok()?;
    let fragment = stat.fragment_size();
    let total = stat.blocks() * fragment;
    let free = stat.blocks_free() * fragment;
    (total > 0).then(|| Disk {
        mount: mount.to_owned(),
        total_bytes: total,
        used_bytes: total.saturating_sub(free),
    })
}

#[cfg(not(unix))]
fn disk_usage(_: &str) -> Option<Disk> {
    None
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
            previous_processes: BTreeMap::new(),
            previous_process_total: None,
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
        self.read_processes(&mut readings);
        if let Ok(mounts) = fs::read_to_string(self.proc_.join("mounts")) {
            readings.disks = real_mounts(&mounts)
                .iter()
                .filter_map(|m| disk_usage(m))
                .collect();
        }
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

    fn read_processes(&mut self, readings: &mut Readings) {
        let Some(total) = fs::read_to_string(self.proc_.join("stat"))
            .ok()
            .as_deref()
            .and_then(parse_cpu_times)
            .map(|t| t.total)
        else {
            return;
        };
        let mut current = BTreeMap::new();
        let mut list = Vec::new();
        let elapsed = self
            .previous_process_total
            .map(|previous| total.saturating_sub(previous));
        if let Ok(entries) = fs::read_dir(&self.proc_) {
            for entry in entries.flatten() {
                let Some(pid) = entry
                    .file_name()
                    .to_str()
                    .and_then(|s| s.parse::<u32>().ok())
                else {
                    continue;
                };
                let Some((name, ticks, rss)) = fs::read_to_string(entry.path().join("stat"))
                    .ok()
                    .as_deref()
                    .and_then(parse_process_stat)
                else {
                    continue;
                };
                current.insert(pid, ticks);
                if let (Some(elapsed), Some(previous)) =
                    (elapsed, self.previous_processes.get(&pid))
                    && elapsed > 0
                {
                    list.push(Process {
                        name,
                        cpu_percent: ticks.saturating_sub(*previous) as f64 * 100.0
                            / elapsed as f64,
                        ram_bytes: rss * PAGE_SIZE,
                        ..Default::default()
                    });
                }
            }
        }
        list.sort_by(|a, b| {
            b.cpu_percent
                .total_cmp(&a.cpu_percent)
                .then(b.ram_bytes.cmp(&a.ram_bytes))
        });
        list.truncate(TOP_PROCESSES);
        readings.processes = list;
        self.previous_processes = current;
        self.previous_process_total = Some(total);
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

    fn process(root: &Path, pid: u32, name: &str, ticks: u64, rss: u64) {
        write(
            root,
            &format!("proc/{pid}/stat"),
            &format!(
                "{pid} ({name}) S 1 1 1 0 -1 4194560 100 0 0 0 {ticks} 0 0 0 20 0 1 0 100 1000 {rss} 18446744073709551615\n"
            ),
        );
    }

    #[test]
    fn ranks_processes_by_cpu_share() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        intel_tree(root, "47500\n");
        process(root, 10, "pycharm64", 100, 1000);
        process(root, 11, "Web Content (x)", 100, 50);
        let mut sensors = sensors(root);
        assert!(sensors.read().processes.is_empty());
        write(root, "proc/stat", "cpu  150 0 150 900 0 0 0 0 0 0\n");
        process(root, 10, "pycharm64", 120, 1000);
        process(root, 11, "Web Content (x)", 150, 50);
        let processes = sensors.read().processes;
        assert_eq!(processes[0].name, "Web Content (x)");
        assert!((processes[0].cpu_percent - 25.0).abs() < 1e-9);
        assert!((processes[1].cpu_percent - 10.0).abs() < 1e-9);
        assert_eq!(processes[1].ram_bytes, 1000 * PAGE_SIZE);
    }

    #[test]
    fn lists_real_mounts_once() {
        let mounts = real_mounts(
            "/dev/nvme0n1p2 / ext4 rw 0 0\n\
             proc /proc proc rw 0 0\n\
             /dev/loop3 /snap/core squashfs ro 0 0\n\
             /dev/sda1 /media/my\\040disk ntfs3 rw 0 0\n\
             /dev/nvme0n1p2 /var/snap ext4 rw 0 0\n",
        );
        assert_eq!(mounts, vec!["/", "/media/my disk"]);
        assert!(parse_process_stat("garbage").is_none());
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
