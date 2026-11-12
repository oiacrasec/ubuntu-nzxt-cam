use crate::devices::{Kind, NZXT_VENDOR};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path, process::Command};

pub const RULE_PATH: &str = "/etc/udev/rules.d/71-nzxt-cam.rules";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Install,
    Remove,
}

impl Action {
    pub fn arg(self) -> &'static str {
        match self {
            Self::Install => "install",
            Self::Remove => "remove",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "install" => Ok(Self::Install),
            "remove" => Ok(Self::Remove),
            _ => bail!("Ação de permissão desconhecida"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Plan {
    pub id: u64,
    pub action: Action,
    /// Comandos e arquivos exibidos antes da confirmação.
    pub steps: Vec<String>,
    pub rule: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub rule_installed: bool,
    /// Regra presente mas diferente da gerada por esta versão.
    pub rule_outdated: bool,
    pub plan: Option<Plan>,
    pub running: bool,
    pub log: String,
    pub error: Option<String>,
}

/// Regra que concede à sessão ativa (`uaccess`) somente os VID/PID suportados.
pub fn rule() -> String {
    let mut text = String::from(
        "# NZXT CAM para Ubuntu: acesso da sessão ativa aos dispositivos suportados.\n\
         # Gerado pelo aplicativo; remova pelo próprio aplicativo em Configurações.\n",
    );
    for kind in Kind::ALL {
        text.push_str(&format!(
            "SUBSYSTEM==\"hidraw\", ATTRS{{idVendor}}==\"{NZXT_VENDOR:04x}\", ATTRS{{idProduct}}==\"{:04x}\", TAG+=\"uaccess\"\n",
            kind.product()
        ));
    }
    text
}

pub fn read_state(path: &Path) -> (bool, bool) {
    match fs::read_to_string(path) {
        Ok(text) => (true, text != rule()),
        Err(_) => (false, false),
    }
}

pub fn plan(id: u64, action: Action, executable: &Path) -> Plan {
    let helper = format!(
        "pkexec {} --permissions-helper {}",
        executable.display(),
        action.arg()
    );
    let steps = match action {
        Action::Install => vec![
            helper,
            format!("gravar {RULE_PATH}"),
            "udevadm control --reload-rules".into(),
            "udevadm trigger --subsystem-match=hidraw --action=change".into(),
        ],
        Action::Remove => vec![
            helper,
            format!("remover {RULE_PATH}"),
            "udevadm control --reload-rules".into(),
            "udevadm trigger --subsystem-match=hidraw --action=change".into(),
        ],
    };
    Plan {
        id,
        action,
        steps,
        rule: (action == Action::Install).then(rule),
    }
}

/// Executa o plano confirmado: uma única autenticação pelo Ubuntu.
pub fn run(action: Action, executable: &Path) -> Result<String> {
    let output = Command::new("pkexec")
        .arg(executable)
        .arg("--permissions-helper")
        .arg(action.arg())
        .output()
        .context("Não foi possível executar pkexec")?;
    let log = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    match output.status.code() {
        Some(0) => Ok(log),
        Some(126) => bail!("Autorização cancelada"),
        Some(127) => bail!("Autorização negada"),
        _ => bail!("Preparação de permissões falhou: {}", log.trim()),
    }
}

/// Modo `--permissions-helper`: etapas fixas, sem ler scripts ou caminhos do usuário.
pub fn helper(action: &str) -> Result<()> {
    let action = Action::parse(action)?;
    #[cfg(unix)]
    if !nix::unistd::Uid::effective().is_root() {
        bail!("O helper de permissões precisa ser executado pelo pkexec");
    }
    let path = Path::new(RULE_PATH);
    match action {
        Action::Install => {
            let dir = path.parent().context("Regra sem diretório")?;
            let temporary = dir.join(".71-nzxt-cam.rules.tmp");
            fs::write(&temporary, rule())?;
            fs::rename(&temporary, path).inspect_err(|_| {
                let _ = fs::remove_file(&temporary);
            })?;
            println!("Regra gravada: {RULE_PATH}");
        }
        Action::Remove => match fs::remove_file(path) {
            Ok(()) => println!("Regra removida: {RULE_PATH}"),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => println!("Regra já ausente"),
            Err(e) => return Err(e).context("Não foi possível remover a regra"),
        },
    }
    for args in [
        &["control", "--reload-rules"][..],
        &["trigger", "--subsystem-match=hidraw", "--action=change"][..],
    ] {
        let status = Command::new("udevadm").args(args).status()?;
        if !status.success() {
            bail!("udevadm {} falhou", args.join(" "));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_covers_exactly_supported_devices() {
        let rule = rule();
        assert!(rule.contains("ATTRS{idVendor}==\"1e71\", ATTRS{idProduct}==\"170e\""));
        assert!(rule.contains("ATTRS{idProduct}==\"1714\""));
        assert_eq!(rule.matches("TAG+=\"uaccess\"").count(), Kind::ALL.len());
        assert!(!rule.contains("MODE="));
    }

    #[test]
    fn plan_lists_commands_before_confirmation() {
        let plan = plan(7, Action::Install, Path::new("/home/u/.local/bin/nzxt-cam"));
        assert_eq!(plan.id, 7);
        assert!(plan.steps[0].starts_with("pkexec /home/u/.local/bin/nzxt-cam"));
        assert!(plan.rule.is_some());
        let removal = super::plan(8, Action::Remove, Path::new("/x"));
        assert!(removal.rule.is_none());
        assert!(Action::parse("rm -rf").is_err());
    }

    #[test]
    fn detects_outdated_rule() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rule");
        assert_eq!(read_state(&path), (false, false));
        fs::write(&path, rule()).unwrap();
        assert_eq!(read_state(&path), (true, false));
        fs::write(&path, "old").unwrap();
        assert_eq!(read_state(&path), (true, true));
    }
}
