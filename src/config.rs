use crate::{curves::ChannelCooling, lighting::ChannelLighting};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TemperatureUnit {
    #[default]
    Celsius,
    Fahrenheit,
}

impl TemperatureUnit {
    #[cfg_attr(not(feature = "gui"), allow(dead_code))]
    pub fn format(self, celsius: Option<f64>) -> String {
        match celsius {
            None => "—".into(),
            Some(c) => match self {
                Self::Celsius => format!("{c:.0} °C"),
                Self::Fahrenheit => format!("{:.0} °F", c * 9.0 / 5.0 + 32.0),
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    pub language: String,
    pub start_minimized: bool,
    pub temperature_unit: TemperatureUnit,
    /// `true`: média dos núcleos; `false`: sensor de pacote (padrão do CAM).
    pub cpu_average_temperature: bool,
    pub mini_mode: bool,
    pub mini_always_on_top: bool,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            language: "pt-BR".into(),
            start_minimized: false,
            temperature_unit: TemperatureUnit::Celsius,
            cpu_average_temperature: false,
            mini_mode: false,
            mini_always_on_top: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GovernorConfig {
    pub interval_ms: u64,
    /// Queda máxima de duty por ciclo, em pontos percentuais.
    pub fall_step: u8,
    /// Duty gravado nos canais sem perfil interno ao encerrar normalmente.
    pub exit_duty: u8,
    /// GPU usada como fonte de temperatura (índice NVML/hwmon).
    pub gpu_index: usize,
}

impl Default for GovernorConfig {
    fn default() -> Self {
        Self {
            interval_ms: 1000,
            fall_step: 5,
            exit_duty: 100,
            gpu_index: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub schema_version: u32,
    pub ui: UiConfig,
    pub governor: GovernorConfig,
    /// Chave: `<id do dispositivo>/<canal>`, por exemplo `kraken_x:SERIAL/pump`.
    pub cooling: BTreeMap<String, ChannelCooling>,
    pub lighting: BTreeMap<String, ChannelLighting>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            schema_version: 1,
            ui: UiConfig::default(),
            governor: GovernorConfig::default(),
            cooling: BTreeMap::new(),
            lighting: BTreeMap::new(),
            extra: BTreeMap::new(),
        }
    }
}

pub fn config_home() -> Result<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .context("HOME e XDG_CONFIG_HOME não estão definidos")
}

pub fn default_path() -> Result<PathBuf> {
    Ok(config_home()?.join("nzxt-cam/config.json"))
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let config = match fs::read(path) {
            Ok(bytes) => serde_json::from_slice::<Self>(&bytes).with_context(|| {
                format!(
                    "Configuração inválida: {} (arquivo preservado)",
                    path.display()
                )
            })?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(e) => return Err(e).context("Não foi possível ler a configuração"),
        };
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<()> {
        if !crate::i18n::valid_tag(&self.ui.language) {
            bail!("Código de idioma inválido");
        }
        if !(250..=10_000).contains(&self.governor.interval_ms) {
            bail!("Intervalo do governador deve estar entre 250 e 10000 ms");
        }
        if !(1..=100).contains(&self.governor.fall_step) {
            bail!("Passo de redução deve estar entre 1 e 100%");
        }
        if !(50..=100).contains(&self.governor.exit_duty) {
            bail!("Rotação de saída deve estar entre 50 e 100%");
        }
        Ok(())
    }

    /// Grava por arquivo temporário e rename; o primeiro salvamento guarda um backup.
    pub fn save(&self, path: &Path) -> Result<()> {
        self.validate()?;
        let dir = path.parent().context("Configuração sem diretório")?;
        fs::create_dir_all(dir)?;
        let backup = path.with_extension("json.backup");
        if path.exists() && !backup.exists() {
            fs::copy(path, &backup).context("Não foi possível criar o backup da configuração")?;
        }
        let mut temporary = tempfile_in(dir)?;
        serde_json::to_writer_pretty(&mut temporary.1, self)?;
        temporary.1.write_all(b"\n")?;
        temporary.1.sync_all()?;
        drop(temporary.1);
        fs::rename(&temporary.0, path).inspect_err(|_| {
            let _ = fs::remove_file(&temporary.0);
        })?;
        Ok(())
    }
}

fn tempfile_in(dir: &Path) -> Result<(PathBuf, fs::File)> {
    for attempt in 0..100u32 {
        let path = dir.join(format!(".config-{}-{attempt}.tmp", std::process::id()));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(file) => return Ok((path, file)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e).context("Não foi possível gravar a configuração"),
        }
    }
    bail!("Não foi possível criar o arquivo temporário da configuração")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::curves::{Profile, Source};

    #[test]
    fn missing_file_uses_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::load(&dir.path().join("config.json")).unwrap();
        assert_eq!(config, Config::default());
    }

    #[test]
    fn invalid_json_is_preserved() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        fs::write(&path, "{ not json").unwrap();
        assert!(Config::load(&path).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "{ not json");
    }

    #[test]
    fn save_keeps_extra_fields_and_backup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        fs::write(
            &path,
            r#"{"future_field":{"a":1},"ui":{"language":"en-US"}}"#,
        )
        .unwrap();
        let mut config = Config::load(&path).unwrap();
        assert_eq!(config.ui.language, "en-US");
        config.cooling.insert(
            "kraken_x:1/pump".into(),
            ChannelCooling {
                profile: Profile::Performance,
                source: Source::Cpu,
            },
        );
        config.save(&path).unwrap();
        config.save(&path).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("future_field"));
        let backup = fs::read_to_string(path.with_extension("json.backup")).unwrap();
        assert!(backup.contains("en-US") && !backup.contains("cooling"));
        assert_eq!(Config::load(&path).unwrap(), config);
        let leftovers = fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .count();
        assert_eq!(leftovers, 0);
    }

    #[test]
    fn rejects_out_of_range_governor_values() {
        for (interval, step, exit) in [(0, 5, 100), (1000, 0, 100), (1000, 5, 49), (1000, 5, 101)] {
            let mut config = Config::default();
            config.governor.interval_ms = interval;
            config.governor.fall_step = step;
            config.governor.exit_duty = exit;
            assert!(config.validate().is_err());
        }
        let mut config = Config::default();
        config.ui.language = "../../etc".into();
        assert!(config.validate().is_err());
    }

    #[test]
    fn formats_temperature_units() {
        assert_eq!(TemperatureUnit::Celsius.format(Some(31.6)), "32 °C");
        assert_eq!(TemperatureUnit::Fahrenheit.format(Some(100.0)), "212 °F");
        assert_eq!(TemperatureUnit::Celsius.format(None), "—");
    }
}
