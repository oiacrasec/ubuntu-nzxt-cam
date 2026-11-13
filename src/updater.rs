//! Atualização explícita pela branch de produção, fora do laço de refrigeração.
use crate::installation::{BuildInfo, Installed, REPOSITORY};
use anyhow::{Context, Result, bail};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    #[default]
    Idle,
    Checking,
    Current,
    Available,
    Installing,
    RestartRequired,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Candidate {
    pub branch: String,
    pub revision: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct State {
    pub build: BuildInfo,
    pub phase: Phase,
    pub candidate: Option<Candidate>,
    pub installed: bool,
    pub message: String,
    pub log: Option<PathBuf>,
}
impl State {
    pub fn busy(&self) -> bool {
        matches!(self.phase, Phase::Checking | Phase::Installing)
    }
    pub fn accept(&mut self, event: Event) {
        match event {
            Event::Progress(message) => self.message = message,
            Event::Checked(result) => match result {
                Ok(candidate) => {
                    let current = candidate.revision == self.build.revision && !self.build.dirty;
                    self.phase = if current {
                        Phase::Current
                    } else {
                        Phase::Available
                    };
                    self.message = if current {
                        "Você está usando a versão mais recente"
                    } else {
                        "Nova atualização disponível"
                    }
                    .into();
                    self.candidate = Some(candidate);
                }
                Err(message) => {
                    self.phase = Phase::Failed;
                    self.message = message;
                }
            },
            Event::Installed(result) => match result {
                Ok(()) => {
                    self.phase = Phase::RestartRequired;
                    self.message =
                        "Atualização instalada. Reinicie para usar a nova versão.".into();
                }
                Err(message) => {
                    self.phase = Phase::Failed;
                    self.message = message;
                }
            },
        }
    }
}

pub enum Event {
    Progress(String),
    Checked(std::result::Result<Candidate, String>),
    Installed(std::result::Result<(), String>),
}
pub struct Job {
    rx: Receiver<Event>,
    cancel: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}
impl Job {
    pub fn check() -> Self {
        Self::spawn(true, |work| production(work, REPOSITORY).map(Some))
    }
    pub fn install(candidate: Candidate, installed: Installed, log: PathBuf) -> Self {
        Self::spawn(false, move |work| {
            install(work, &candidate, &installed, &log).map(|()| None)
        })
    }
    fn spawn(
        checking: bool,
        task: impl FnOnce(&Work) -> Result<Option<Candidate>> + Send + 'static,
    ) -> Self {
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let thread = thread::spawn(move || {
            let work = Work { tx, cancel: flag };
            let event = match task(&work) {
                Ok(Some(candidate)) => Event::Checked(Ok(candidate)),
                Ok(None) => Event::Installed(Ok(())),
                Err(error) if checking => Event::Checked(Err(format!("{error:#}"))),
                Err(error) => Event::Installed(Err(format!("{error:#}"))),
            };
            let _ = work.tx.send(event);
        });
        Self {
            rx,
            cancel,
            thread: Some(thread),
        }
    }
    pub fn poll(&self) -> Option<Event> {
        self.rx.try_recv().ok()
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
struct Work {
    tx: Sender<Event>,
    cancel: Arc<AtomicBool>,
}
impl Work {
    fn phase(&self, text: &str) {
        let _ = self.tx.send(Event::Progress(text.into()));
    }
    fn run(&self, command: &mut Command, timeout: Duration, log: Option<&Path>) -> Result<String> {
        let capture = tempfile::NamedTempFile::new()?;
        let stderr = if let Some(path) = log {
            fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)?
        } else {
            capture.reopen()?
        };
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        command
            .stdin(Stdio::null())
            .stdout(capture.reopen()?)
            .stderr(stderr)
            .env("GIT_TERMINAL_PROMPT", "0");
        let mut child = command
            .spawn()
            .context("Não foi possível executar a etapa de atualização")?;
        let started = Instant::now();
        let status = loop {
            if let Some(status) = child.try_wait()? {
                break status;
            }
            if self.cancel.load(Ordering::Relaxed) || started.elapsed() > timeout {
                #[cfg(unix)]
                {
                    let _ = nix::sys::signal::killpg(
                        nix::unistd::Pid::from_raw(child.id() as i32),
                        nix::sys::signal::Signal::SIGKILL,
                    );
                }
                #[cfg(not(unix))]
                {
                    let _ = child.kill();
                }
                let _ = child.wait();
                bail!("Atualização cancelada ou tempo limite atingido");
            }
            thread::sleep(Duration::from_millis(80));
        };
        let mut text = String::new();
        capture
            .reopen()?
            .take(64 * 1024)
            .read_to_string(&mut text)?;
        if !status.success() {
            let tail = log.and_then(|path| tail(path).ok()).unwrap_or(text.clone());
            bail!(
                "A atualização não pôde concluir esta etapa: {}",
                tail.trim()
            );
        }
        Ok(text)
    }
}

fn tail(path: &Path) -> Result<String> {
    use std::io::{Seek, SeekFrom};
    let mut file = fs::File::open(path)?;
    let size = file.metadata()?.len();
    file.seek(SeekFrom::Start(size.saturating_sub(4096)))?;
    let mut bytes = Vec::new();
    file.take(4096).read_to_end(&mut bytes)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn valid_revision(revision: &str) -> bool {
    revision.len() == 40 && revision.bytes().all(|byte| byte.is_ascii_hexdigit())
}
fn parse_production(text: &str) -> Result<Candidate> {
    let branch = text
        .lines()
        .find_map(|line| {
            line.strip_prefix("ref: refs/heads/")
                .and_then(|value| value.strip_suffix("\tHEAD"))
        })
        .filter(|branch| matches!(*branch, "main" | "master"))
        .context("A branch de produção precisa ser main ou master")?;
    let revision = text
        .lines()
        .find_map(|line| {
            line.strip_suffix("\tHEAD")
                .filter(|value| valid_revision(value))
        })
        .context("Revisão de produção inválida")?;
    Ok(Candidate {
        branch: branch.into(),
        revision: revision.into(),
    })
}
fn production(work: &Work, repository: &str) -> Result<Candidate> {
    work.phase("Verificando atualizações…");
    let text = work
        .run(
            Command::new("git").args(["ls-remote", "--symref", repository, "HEAD"]),
            Duration::from_secs(45),
            None,
        )
        .context(
            "Não foi possível consultar atualizações. Verifique a conexão e tente novamente.",
        )?;
    parse_production(&text)
}
pub fn log_path() -> Result<PathBuf> {
    Ok(cache()?.join("update.log"))
}
fn cache() -> Result<PathBuf> {
    let root = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
        .context("HOME não definido")?
        .join("nzxt-cam/updates");
    fs::create_dir_all(&root)?;
    Ok(root)
}
fn verify_build(build: &BuildInfo, candidate: &Candidate, installed: &Installed) -> Result<()> {
    if build.application != "nzxt-cam"
        || build.revision != candidate.revision
        || build.dirty
        || build.gui != installed.build.gui
        || build.tray != installed.build.tray
        || build.nvidia != installed.build.nvidia
    {
        bail!("A nova compilação não corresponde à atualização selecionada");
    }
    Ok(())
}
fn install(work: &Work, candidate: &Candidate, installed: &Installed, log: &Path) -> Result<()> {
    if !cfg!(unix) {
        bail!("A atualização operacional só está disponível no Linux");
    }
    if !valid_revision(&candidate.revision)
        || !matches!(candidate.branch.as_str(), "main" | "master")
    {
        bail!("Candidato de atualização inválido");
    }
    let cache = cache()?;
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(cache.join("update.lock"))?;
    lock.try_lock_exclusive()
        .context("Outra atualização está em andamento")?;
    fs::write(log, b"NZXT CAM update\n")?;
    let current = production(work, REPOSITORY)?;
    if current.branch != candidate.branch {
        bail!("A origem da atualização mudou; verifique novamente");
    }
    let source = tempfile::Builder::new()
        .prefix("source-")
        .tempdir_in(&cache)?;
    work.phase("Baixando a atualização…");
    work.run(
        Command::new("git")
            .args([
                "clone",
                "--no-checkout",
                "--single-branch",
                "--no-tags",
                "--branch",
                &candidate.branch,
                REPOSITORY,
            ])
            .arg(source.path()),
        Duration::from_secs(180),
        Some(log),
    )?;
    work.run(
        Command::new("git").arg("-C").arg(source.path()).args([
            "checkout",
            "--detach",
            &candidate.revision,
        ]),
        Duration::from_secs(30),
        Some(log),
    )?;
    if valid_revision(&installed.build.revision) {
        work.run(
            Command::new("git").arg("-C").arg(source.path()).args([
                "merge-base",
                "--is-ancestor",
                &installed.build.revision,
                &candidate.revision,
            ]),
            Duration::from_secs(30),
            Some(log),
        )
        .context("A revisão selecionada não continua a instalação atual")?;
    }
    let target = cache.join("build");
    let cargo = std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join(".cargo/bin/cargo"))
        .filter(|path| path.is_file())
        .unwrap_or_else(|| "cargo".into());
    let mut features = Vec::new();
    if installed.build.gui {
        features.push("gui");
    }
    if installed.build.tray {
        features.push("tray");
    }
    if installed.build.nvidia {
        features.push("nvidia");
    }
    work.phase("Preparando a nova versão. Isso pode levar alguns minutos…");
    work.run(
        Command::new(cargo)
            .current_dir(source.path())
            .args([
                "build",
                "--locked",
                "--release",
                "--no-default-features",
                "--features",
                &features.join(","),
                "--target-dir",
            ])
            .arg(&target)
            .env("CARGO_INCREMENTAL", "0"),
        Duration::from_secs(3600),
        Some(log),
    )?;
    let binary = target.join("release/nzxt-cam");
    let info = work.run(
        Command::new(&binary).arg("--build-info"),
        Duration::from_secs(15),
        Some(log),
    )?;
    let build: BuildInfo =
        serde_json::from_str(&info).context("A nova versão não informou sua identidade")?;
    verify_build(&build, candidate, installed)?;
    let checksum = crate::installation::checksum(&binary)?;
    // Backup do executável e do registro antes da única etapa que publica arquivos.
    let parent = installed
        .launcher()
        .parent()
        .context("Instalação sem diretório")?
        .to_owned();
    let backup = tempfile::NamedTempFile::new_in(&parent)?;
    fs::copy(installed.launcher(), backup.path())?;
    let marker = parent.join("nzxt-cam.installation.json");
    let old_marker = fs::read(&marker)?;
    work.phase("Instalando a atualização…");
    let publish = (|| -> Result<()> {
        work.run(
            Command::new("bash")
                .arg(source.path().join("install-desktop-app.sh"))
                .args(["--no-build", "--prefix"])
                .arg(&installed.prefix)
                .env("CARGO_TARGET_DIR", &target),
            Duration::from_secs(60),
            Some(log),
        )?;
        let published: Installed = serde_json::from_slice(&fs::read(&marker)?)?;
        if published.build != build
            || published.sha256 != checksum
            || crate::installation::checksum(&installed.launcher())? != checksum
        {
            bail!("Não foi possível confirmar a integridade da instalação publicada");
        }
        Ok(())
    })();
    if let Err(error) = publish {
        fs::rename(backup.path(), installed.launcher())
            .context("Falha ao restaurar o executável anterior")?;
        fs::write(marker, old_marker).context("Falha ao restaurar o registro anterior")?;
        return Err(error);
    }
    Ok(())
}

pub fn restart(installed: &Installed, config: &Path, gui: bool) -> Result<()> {
    let mut command = Command::new(installed.launcher());
    command
        .args([
            "--restart-after",
            &std::process::id().to_string(),
            "--config",
        ])
        .arg(config)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if !gui {
        command.arg("--headless");
    }
    command
        .spawn()
        .context("Não foi possível reiniciar o NZXT CAM")?;
    Ok(())
}
pub fn wait_for_parent(pid: u32) -> Result<()> {
    if pid < 2 || pid > i32::MAX as u32 {
        bail!("Processo de reinício inválido");
    }
    #[cfg(not(unix))]
    {
        bail!("O reinício de atualização só está disponível no Linux");
    }
    #[cfg(unix)]
    {
        let start = Instant::now();
        loop {
            match nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid as i32), None) {
                Err(nix::errno::Errno::ESRCH) => return Ok(()),
                Ok(()) => {}
                Err(error) => return Err(error.into()),
            }
            if start.elapsed() > Duration::from_secs(20) {
                bail!("A instância anterior não encerrou a tempo");
            }
            thread::sleep(Duration::from_millis(100));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_only_production_branch_and_full_commit_identity() {
        let revision = "a".repeat(40);
        let candidate =
            parse_production(&format!("ref: refs/heads/main\tHEAD\n{revision}\tHEAD")).unwrap();
        assert_eq!(candidate.revision, revision);
        assert!(parse_production(&format!("ref: refs/heads/dev\tHEAD\n{revision}\tHEAD")).is_err());
        assert!(parse_production("ref: refs/heads/main\tHEAD\ninvalid\tHEAD").is_err());
        let build = BuildInfo {
            revision,
            dirty: false,
            ..Default::default()
        };
        let installed = Installed {
            prefix: "/fixture".into(),
            build: build.clone(),
            sha256: String::new(),
        };
        verify_build(&build, &candidate, &installed).unwrap();
        let wrong = BuildInfo {
            dirty: true,
            ..build
        };
        assert!(verify_build(&wrong, &candidate, &installed).is_err());
    }
    #[test]
    fn tracks_update_errors_and_requires_explicit_restart() {
        let mut state = State::default();
        state.accept(Event::Installed(Err("build failed".into())));
        assert_eq!(state.phase, Phase::Failed);
        state.accept(Event::Installed(Ok(())));
        assert_eq!(state.phase, Phase::RestartRequired);
        assert!(wait_for_parent(0).is_err());
    }

    #[test]
    fn checks_a_local_repository_in_a_worker_without_network() {
        let repo = tempfile::tempdir().unwrap();
        assert!(
            Command::new("git")
                .args(["init", "--initial-branch=main"])
                .arg(repo.path())
                .output()
                .unwrap()
                .status
                .success()
        );
        assert!(
            Command::new("git")
                .arg("-C")
                .arg(repo.path())
                .args([
                    "-c",
                    "user.name=Fixture",
                    "-c",
                    "user.email=fixture@example.invalid",
                    "commit",
                    "--allow-empty",
                    "-m",
                    "fixture",
                ])
                .output()
                .unwrap()
                .status
                .success()
        );
        let path = repo.path().to_string_lossy().into_owned();
        let job = Job::spawn(true, move |work| production(work, &path).map(Some));
        let start = Instant::now();
        loop {
            if let Some(Event::Checked(result)) = job.poll() {
                assert_eq!(result.unwrap().branch, "main");
                break;
            }
            assert!(start.elapsed() < Duration::from_secs(10));
            thread::sleep(Duration::from_millis(10));
        }
    }

    #[cfg(unix)]
    #[test]
    fn cancels_only_the_update_process_group_on_timeout() {
        let (tx, _) = mpsc::channel();
        let work = Work {
            tx,
            cancel: Arc::new(AtomicBool::new(false)),
        };
        let start = Instant::now();
        assert!(
            work.run(
                Command::new("sh").args(["-c", "sleep 30 & wait"]),
                Duration::from_millis(100),
                None
            )
            .is_err()
        );
        assert!(start.elapsed() < Duration::from_secs(3));
    }
}
