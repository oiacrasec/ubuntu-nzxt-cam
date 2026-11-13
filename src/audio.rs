//! Captura do som que o sistema toca, para os modos de áudio da iluminação
//! (Nivelado, Sincronização, Graves e Ganho).
//!
//! O CAM Windows lê o áudio do sistema; aqui a leitura vem do monitor da saída
//! padrão do PipeWire/PulseAudio por `parec` (pacote `pulseaudio-utils`) ou, na
//! falta dele, por `pw-record` (pacote `pipewire-bin`). O fluxo é PCM de 16 bits,
//! mono, 48 kHz; a análise produz o volume e a energia dos graves, de 0 a 1.

use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const RATE: f32 = 48_000.0;
/// 20 ms por bloco de análise.
const BLOCK: usize = 960;
/// Frequência de corte do filtro de graves.
const BASS_HZ: f32 = 150.0;
/// Faixa útil: abaixo de -60 dBFS conta como silêncio.
const FLOOR_DB: f32 = -60.0;
/// Queda por bloco depois de um pico, para o medidor não piscar.
const RELEASE: f32 = 0.85;
/// Sem blocos novos por este tempo, o som é tratado como silêncio.
const STALE: Duration = Duration::from_millis(500);

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Levels {
    pub level: f32,
    pub bass: f32,
}

/// Estado da análise entre blocos.
#[derive(Debug, Default)]
pub struct Analyzer {
    low: f32,
    levels: Levels,
}

fn loudness(rms: f32) -> f32 {
    if rms <= 0.0 {
        return 0.0;
    }
    let db = 20.0 * (rms / 32768.0).log10();
    ((db - FLOOR_DB) / -FLOOR_DB).clamp(0.0, 1.0)
}

impl Analyzer {
    /// Analisa um bloco de amostras; o resultado sobe na hora e cai aos poucos.
    pub fn push(&mut self, samples: &[i16]) -> Levels {
        if samples.is_empty() {
            return self.levels;
        }
        let alpha = 1.0 - (-std::f32::consts::TAU * BASS_HZ / RATE).exp();
        let (mut all, mut low) = (0.0f32, 0.0f32);
        for &sample in samples {
            let x = f32::from(sample);
            self.low += alpha * (x - self.low);
            all += x * x;
            low += self.low * self.low;
        }
        let n = samples.len() as f32;
        let level = loudness((all / n).sqrt());
        let bass = loudness((low / n).sqrt());
        self.levels = Levels {
            level: level.max(self.levels.level * RELEASE),
            bass: bass.max(self.levels.bass * RELEASE),
        };
        self.levels
    }
}

/// Programas tentados em ordem; o primeiro que entregar áudio é usado.
fn commands() -> Vec<Command> {
    let mut parec = Command::new("parec");
    parec.args([
        "--device=@DEFAULT_MONITOR@",
        "--format=s16le",
        "--rate=48000",
        "--channels=1",
        "--latency-msec=20",
    ]);
    let mut pw = Command::new("pw-record");
    pw.args([
        "--raw",
        "-P",
        "{ stream.capture.sink=true }",
        "--format=s16",
        "--rate=48000",
        "--channels=1",
        "-",
    ]);
    vec![parec, pw]
}

#[derive(Default)]
struct Shared {
    levels: Levels,
    updated: Option<Instant>,
    error: Option<String>,
}

/// Captura em segundo plano; termina o processo ao ser descartada.
pub struct Capture {
    shared: Arc<Mutex<Shared>>,
    child: Arc<Mutex<Option<Child>>>,
    stop: Arc<AtomicBool>,
}

impl Capture {
    pub fn start() -> Self {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let child = Arc::new(Mutex::new(None));
        let stop = Arc::new(AtomicBool::new(false));
        let (s, c, halt) = (shared.clone(), child.clone(), stop.clone());
        thread::spawn(move || run(&s, &c, &halt));
        Self {
            shared,
            child,
            stop,
        }
    }

    /// Volume e graves atuais; silêncio se a captura parou ou ainda não começou.
    pub fn levels(&self) -> Levels {
        let shared = self.shared.lock().unwrap_or_else(|e| e.into_inner());
        match shared.updated {
            Some(time) if time.elapsed() < STALE => shared.levels,
            _ => Levels::default(),
        }
    }

    /// Motivo da falha quando nenhum programa de captura funcionou.
    pub fn error(&self) -> Option<String> {
        let shared = self.shared.lock().unwrap_or_else(|e| e.into_inner());
        shared.error.clone()
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(mut child) = self.child.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn run(shared: &Mutex<Shared>, slot: &Mutex<Option<Child>>, stop: &AtomicBool) {
    let mut failures = Vec::new();
    for mut command in commands() {
        if stop.load(Ordering::SeqCst) {
            return;
        }
        let program = command.get_program().to_string_lossy().into_owned();
        let spawned = command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();
        let mut child = match spawned {
            Ok(child) => child,
            Err(error) => {
                failures.push(format!("{program}: {error}"));
                continue;
            }
        };
        let Some(mut stdout) = child.stdout.take() else {
            continue;
        };
        *slot.lock().unwrap_or_else(|e| e.into_inner()) = Some(child);
        let mut analyzer = Analyzer::default();
        let mut bytes = vec![0u8; BLOCK * 2];
        let mut received = false;
        while !stop.load(Ordering::SeqCst) && stdout.read_exact(&mut bytes).is_ok() {
            received = true;
            let samples: Vec<i16> = (0..BLOCK)
                .map(|i| i16::from_le_bytes([bytes[2 * i], bytes[2 * i + 1]]))
                .collect();
            let levels = analyzer.push(&samples);
            let mut shared = shared.lock().unwrap_or_else(|e| e.into_inner());
            shared.levels = levels;
            shared.updated = Some(Instant::now());
            shared.error = None;
        }
        if let Some(mut child) = slot.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        if received || stop.load(Ordering::SeqCst) {
            return;
        }
        failures.push(format!("{program}: não entregou áudio"));
    }
    shared.lock().unwrap_or_else(|e| e.into_inner()).error = Some(format!(
        "Modos de áudio sem captura do som do sistema ({}). Instale pulseaudio-utils",
        failures.join("; ")
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(hz: f32, amplitude: f32) -> Vec<i16> {
        (0..BLOCK)
            .map(|i| {
                let t = i as f32 / RATE;
                (amplitude * (std::f32::consts::TAU * hz * t).sin()) as i16
            })
            .collect()
    }

    #[test]
    fn silence_is_zero_and_loud_tones_are_high() {
        let mut analyzer = Analyzer::default();
        assert_eq!(analyzer.push(&[0; BLOCK]), Levels::default());
        let loud = analyzer.push(&tone(1000.0, 30000.0));
        assert!(loud.level > 0.9, "{loud:?}");
    }

    #[test]
    fn bass_tracks_low_frequencies_only() {
        let mut low = Analyzer::default();
        let mut high = Analyzer::default();
        for _ in 0..5 {
            low.push(&tone(50.0, 20000.0));
            high.push(&tone(5000.0, 20000.0));
        }
        let (low, high) = (low.levels, high.levels);
        assert!(low.bass > 0.8, "{low:?}");
        assert!(high.bass < low.bass - 0.3, "{high:?}");
        assert!((low.level - high.level).abs() < 0.05);
    }

    #[test]
    fn levels_fall_gradually_after_a_peak() {
        let mut analyzer = Analyzer::default();
        let peak = analyzer.push(&tone(1000.0, 30000.0)).level;
        let after = analyzer.push(&[0; BLOCK]).level;
        assert!((after - peak * RELEASE).abs() < 1e-6);
    }
}
