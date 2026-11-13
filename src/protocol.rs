//! Relatórios HID dos dispositivos USB v1 da NZXT, sem E/S.
//!
//! Cada formato foi confirmado em três fontes: o código do NZXT CAM 4.76.8
//! (`index.node`), a documentação pública de protocolo e relatórios reais lidos
//! dos dispositivos do usuário. Ver `docs/REFERENCIA_WINDOWS.md#protocolo`.
use crate::{
    curves::{self, Point},
    lighting::{ChannelLighting, Mode, Rgb},
};
use anyhow::{Result, bail};

/// Todo relatório de saída tem 65 bytes: número do relatório + 64, com zeros.
pub const REPORT_LEN: usize = 65;
/// Comando de velocidade `02 4d` (CAM: `mov word ptr [..], 0x4d02`).
const SPEED: [u8; 2] = [0x02, 0x4d];

/// Grade de temperaturas aceita pelo firmware do Kraken X para curvas:
/// 20–49 °C de 1 em 1, 50–58 °C de 2 em 2 e 60 °C (36 pontos).
pub fn kraken_grid() -> Vec<u8> {
    (20..50).chain((50..=58).step_by(2)).chain([60]).collect()
}

fn report(prefix: &[u8], payload: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; REPORT_LEN];
    out[..prefix.len()].copy_from_slice(prefix);
    out[prefix.len()..prefix.len() + payload.len()].copy_from_slice(payload);
    out
}

/// Rotação fixa: `02 4d CANAL 00 DUTY` (Kraken com firmware < 3 e Smart Device).
pub fn fixed_speed(channel: u8, duty: u8) -> Vec<u8> {
    report(&SPEED, &[channel, 0x00, duty.min(100)])
}

/// Curva do Kraken X: uma mensagem `02 4d (0x80+BASE+i) TEMP DUTY` por ponto da grade.
/// BASE: fan `0x00`, bomba `0x40`. A curva sempre chega a 100% a 60 °C.
pub fn kraken_profile(base: u8, points: &[Point], floor: u8) -> Result<Vec<Vec<u8>>> {
    if base != 0x00 && base != 0x40 {
        bail!("Canal de curva inválido");
    }
    let grid = kraken_grid();
    let mut out = Vec::with_capacity(grid.len());
    for (index, temperature) in grid.iter().enumerate() {
        let mut duty = curves::duty_at(points, f64::from(*temperature)).clamp(floor, 100);
        if *temperature >= 60 {
            duty = 100;
        }
        let channel = 0x80 + base + index as u8;
        out.push(report(&SPEED, &[channel, *temperature, duty]));
    }
    Ok(out)
}

#[derive(Clone, Debug, PartialEq)]
pub struct KrakenStatus {
    pub liquid: f64,
    pub fan_rpm: u16,
    pub pump_rpm: u16,
    pub firmware: (u8, u8, u8, u8),
}

/// Relatório `04`: líquido = b1 + b2 × 0,1; fan = b3..b4 e bomba = b5..b6 (big-endian);
/// firmware em b11..b14. Exemplo real: `04 21 01 00 00 07 f2 … 06 00 00 02`.
pub fn kraken_status(report: &[u8]) -> Option<KrakenStatus> {
    if report.len() < 17 || report[0] != 0x04 {
        return None;
    }
    let liquid = f64::from(report[1]) + f64::from(report[2]) * 0.1;
    if !(0.0..=100.0).contains(&liquid) || report[2] > 9 {
        return None;
    }
    Some(KrakenStatus {
        liquid,
        fan_rpm: u16::from_be_bytes([report[3], report[4]]),
        pump_rpm: u16::from_be_bytes([report[5], report[6]]),
        firmware: (report[11], report[12], report[13], report[14]),
    })
}

#[derive(Clone, Debug, PartialEq)]
pub struct SmartFanStatus {
    /// 0, 1 ou 2.
    pub index: u8,
    pub rpm: u16,
    pub voltage: f64,
    pub current: f64,
    /// 0 = nenhum fan, 1 = DC, 2 = PWM.
    pub mode: u8,
    pub noise: u8,
}

/// Relatório `04` de 21 bytes, um por fan: ruído b1, RPM b3..b4 (big-endian),
/// tensão b7 + b8/100, corrente b9 + b10/100, fan = (b15 >> 4) e modo = b15 & 3.
pub fn smart_status(report: &[u8]) -> Option<SmartFanStatus> {
    if report.len() < 16 || report[0] != 0x04 {
        return None;
    }
    let index = report[15] >> 4;
    if index > 2 {
        return None;
    }
    Some(SmartFanStatus {
        index,
        rpm: u16::from_be_bytes([report[3], report[4]]),
        voltage: f64::from(report[7]) + f64::from(report[8]) / 100.0,
        current: f64::from(report[9]) + f64::from(report[10]) / 100.0,
        mode: report[15] & 0x03,
        noise: report[1],
    })
}

/// Inicialização do Smart Device V1: detectar acessórios e iniciar relatórios.
pub fn smart_init() -> [Vec<u8>; 2] {
    [report(&[0x01, 0x5c], &[]), report(&[0x01, 0x5d], &[])]
}

fn speed_byte(lighting: &ChannelLighting, step: usize) -> u8 {
    let size = match lighting.mode {
        Mode::Marquee => (lighting.led_size.clamp(3, 6) - 3) << 3,
        _ => 0,
    };
    lighting.speed.index() | size | ((step as u8 & 0x07) << 5)
}

/// Id de modo do Kraken X e se o modo só existe no anel.
pub fn kraken_mode(mode: Mode) -> Option<(u8, bool)> {
    Some(match mode {
        Mode::Off | Mode::Fixed => (0x00, false),
        Mode::Fading => (0x01, false),
        Mode::SpectrumWave => (0x02, false),
        Mode::Marquee => (0x03, true),
        Mode::CoveringMarquee => (0x04, true),
        Mode::Alternating => (0x05, true),
        Mode::Breathing => (0x06, false),
        Mode::Pulse => (0x07, false),
        Mode::TaiChi => (0x08, true),
        Mode::WaterCooler => (0x09, true),
        Mode::Loading => (0x0a, true),
        Mode::Wings => (0x0c, true),
        _ => return None,
    })
}

/// Iluminação do Kraken X: `02 4c CANAL|dir|mov MODO VEL` + logo (GRB) + 8 LEDs do anel.
/// Canal: logo `0x01`, anel `0x02`. Uma mensagem por cor (passo da animação).
pub fn kraken_lighting(channel: u8, lighting: &ChannelLighting) -> Result<Vec<Vec<u8>>> {
    let Some((mode, ring_only)) = kraken_mode(lighting.mode) else {
        bail!("Modo {} não existe no Kraken X", lighting.mode.label());
    };
    if ring_only && channel != 0x02 {
        bail!("Modo {} só existe no anel", lighting.mode.label());
    }
    let mut flags = channel;
    if lighting.backwards {
        flags |= 0x10;
    }
    if lighting.moving && lighting.mode == Mode::Alternating {
        flags |= 0x08;
    }
    let colors: Vec<Rgb> = match lighting.mode {
        Mode::Off => vec![Rgb(0, 0, 0)],
        _ if lighting.colors.is_empty() => vec![Rgb(0, 0, 0)],
        _ => lighting.colors.clone(),
    };
    if colors.len() > 8 {
        bail!("O Kraken X aceita no máximo 8 cores por animação");
    }
    let mut out = Vec::new();
    for (step, color) in colors.iter().enumerate() {
        let mut payload = vec![flags, mode, speed_byte(lighting, step)];
        payload.extend([color.1, color.0, color.2]);
        for _ in 0..8 {
            payload.extend([color.0, color.1, color.2]);
        }
        out.push(report(&[0x02, 0x4c], &payload));
    }
    Ok(out)
}

/// Id de modo do Smart Device V1 (difere do Kraken: pulse 0x06, breathing 0x07).
pub fn smart_mode(mode: Mode) -> Option<u8> {
    Some(match mode {
        Mode::Off | Mode::Fixed => 0x00,
        Mode::Fading => 0x01,
        Mode::SpectrumWave => 0x02,
        Mode::Marquee => 0x03,
        Mode::CoveringMarquee => 0x04,
        Mode::Alternating => 0x05,
        Mode::Pulse => 0x06,
        Mode::Breathing => 0x07,
        Mode::Candle => 0x09,
        Mode::Wings => 0x0c,
        _ => return None,
    })
}

const SMART_LEDS: usize = 40;

/// Iluminação do Smart Device V1: por passo, `02 4b MODO VARIANTE VEL` + 57 bytes GRB,
/// depois `03` + 63 bytes restantes (40 LEDs × 3).
pub fn smart_lighting(lighting: &ChannelLighting) -> Result<Vec<Vec<u8>>> {
    let Some(mode) = smart_mode(lighting.mode) else {
        bail!("Modo {} não existe no Smart Device", lighting.mode.label());
    };
    let mut variant = 0u8;
    if lighting.backwards {
        variant |= 0x10;
    }
    if lighting.moving && lighting.mode == Mode::Alternating {
        variant |= 0x08;
    }
    let colors: Vec<Rgb> = match lighting.mode {
        Mode::Off => vec![Rgb(0, 0, 0)],
        _ if lighting.colors.is_empty() => vec![Rgb(0, 0, 0)],
        _ => lighting.colors.clone(),
    };
    if colors.len() > 8 {
        bail!("O Smart Device aceita no máximo 8 cores por animação");
    }
    let mut out = Vec::new();
    for (step, color) in colors.iter().enumerate() {
        let mut leds = Vec::with_capacity(SMART_LEDS * 3);
        for _ in 0..SMART_LEDS {
            leds.extend([color.1, color.0, color.2]);
        }
        let mut first = vec![mode, variant, speed_byte(lighting, step)];
        first.extend(&leds[..57]);
        out.push(report(&[0x02, 0x4b], &first));
        out.push(report(&[0x03], &leds[57..]));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lighting::Speed;

    fn hex(text: &str) -> Vec<u8> {
        text.split_whitespace()
            .map(|b| u8::from_str_radix(b, 16).unwrap())
            .collect()
    }

    /// Relatório real do Kraken X do usuário (Windows, 12/11/2026).
    const KRAKEN_REAL: &str = "04 21 01 00 00 07 f2 00 00 00 e3 06 00 00 02 1e 00 06 02 00 97 89 b2 ce 9b 89 b2 ce 9f 89 b2 ce a3 89 b2 ce a7 89 b2 ce 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00";

    #[test]
    fn decodes_real_kraken_report() {
        let status = kraken_status(&hex(KRAKEN_REAL)).unwrap();
        assert!((status.liquid - 33.1).abs() < 1e-9);
        assert_eq!(status.fan_rpm, 0);
        assert_eq!(status.pump_rpm, 2034);
        assert_eq!(status.firmware, (6, 0, 0, 2));
        let other =
            kraken_status(&hex("04 20 09 00 00 07 ec 00 00 00 e3 06 00 00 02 1e 00")).unwrap();
        assert!((other.liquid - 32.9).abs() < 1e-9);
        assert_eq!(other.pump_rpm, 2028);
    }

    #[test]
    fn rejects_short_foreign_or_implausible_kraken_reports() {
        assert!(kraken_status(&[]).is_none());
        assert!(kraken_status(&hex("04 21 01 00 00 07")).is_none());
        let mut report = hex(KRAKEN_REAL);
        report[0] = 0x05;
        assert!(kraken_status(&report).is_none());
        report[0] = 0x04;
        report[1] = 0xff;
        assert!(kraken_status(&report).is_none());
        report[1] = 0x21;
        report[2] = 0x0a;
        assert!(kraken_status(&report).is_none());
    }

    #[test]
    fn decodes_real_smart_device_reports() {
        let fan2 = smart_status(&hex(
            "04 52 00 05 1a 00 00 09 55 00 0d 01 00 00 07 11 01 02 00 1e 00",
        ))
        .unwrap();
        assert_eq!(
            (fan2.index, fan2.rpm, fan2.mode, fan2.noise),
            (1, 1306, 1, 0x52)
        );
        assert!((fan2.voltage - 9.85).abs() < 1e-9);
        assert!((fan2.current - 0.13).abs() < 1e-9);
        let fan3 = smart_status(&hex(
            "04 58 00 05 3d 00 00 09 63 00 0f 01 00 00 07 21 00 02 00 1e 00",
        ))
        .unwrap();
        assert_eq!((fan3.index, fan3.rpm, fan3.mode), (2, 1341, 1));
        let fan1 = smart_status(&hex(
            "04 56 00 00 00 00 00 0c 12 00 00 01 00 00 07 00 00 02 00 1e 00",
        ))
        .unwrap();
        assert_eq!((fan1.index, fan1.rpm, fan1.mode), (0, 0, 0));
        assert!(smart_status(&hex("04 56 00 00 00 00 00 0c 12 00 00 01 00 00 07 30")).is_none());
        assert!(smart_status(&hex("03 56")).is_none());
    }

    #[test]
    fn fixed_speed_matches_cam_layout() {
        let report = fixed_speed(0x40, 60);
        assert_eq!(report.len(), REPORT_LEN);
        assert_eq!(&report[..5], &[0x02, 0x4d, 0x40, 0x00, 60]);
        assert!(report[5..].iter().all(|b| *b == 0));
        assert_eq!(fixed_speed(0x02, 250)[4], 100);
    }

    #[test]
    fn kraken_profile_uses_grid_channels_floor_and_ceiling() {
        let grid = kraken_grid();
        assert_eq!(grid.len(), 36);
        assert_eq!(grid[0], 20);
        assert_eq!(grid[29], 49);
        assert_eq!(&grid[30..], &[50, 52, 54, 56, 58, 60]);
        let points = vec![
            Point {
                temperature: 0.0,
                duty: 0,
            },
            Point {
                temperature: 100.0,
                duty: 40,
            },
        ];
        let pump = kraken_profile(0x40, &points, 50).unwrap();
        assert_eq!(pump.len(), 36);
        assert_eq!(&pump[0][..5], &[0x02, 0x4d, 0xc0, 20, 50]);
        assert_eq!(&pump[35][..5], &[0x02, 0x4d, 0xe3, 60, 100]);
        let fan = kraken_profile(0x00, &points, 25).unwrap();
        assert_eq!(fan[0][2], 0x80);
        assert_eq!(fan[35][2], 0xa3);
        assert!(fan.iter().all(|r| r.len() == REPORT_LEN && r[4] <= 100));
        // Os canais da bomba e do fan nunca se sobrepõem.
        assert!(fan.iter().all(|r| r[2] < 0xc0));
        assert!(kraken_profile(0x80, &points, 25).is_err());
    }

    #[test]
    fn kraken_lighting_layout() {
        let lighting = ChannelLighting {
            mode: Mode::Breathing,
            colors: vec![Rgb(0x10, 0x20, 0x30), Rgb(1, 2, 3)],
            speed: Speed::Fast,
            backwards: true,
            ..Default::default()
        };
        let reports = kraken_lighting(0x02, &lighting).unwrap();
        assert_eq!(reports.len(), 2);
        assert_eq!(
            &reports[0][..8],
            &[0x02, 0x4c, 0x12, 0x06, 0x03, 0x20, 0x10, 0x30]
        );
        assert_eq!(&reports[0][8..11], &[0x10, 0x20, 0x30]);
        assert_eq!(reports[1][4], 0x03 | (1 << 5));
        let marquee = ChannelLighting {
            mode: Mode::Marquee,
            led_size: 5,
            ..Default::default()
        };
        assert!(kraken_lighting(0x01, &marquee).is_err());
        assert_eq!(kraken_lighting(0x02, &marquee).unwrap()[0][4], 0x02 | 0x10);
        assert!(
            kraken_lighting(
                0x01,
                &ChannelLighting {
                    mode: Mode::Candle,
                    ..Default::default()
                }
            )
            .is_err()
        );
    }

    #[test]
    fn smart_lighting_splits_in_two_reports() {
        let reports = smart_lighting(&ChannelLighting::default()).unwrap();
        assert_eq!(reports.len(), 2);
        assert_eq!(&reports[0][..5], &[0x02, 0x4b, 0x00, 0x00, 0x02]);
        assert_eq!(&reports[0][5..8], &[0x00, 0x7f, 0xff]);
        assert_eq!(reports[1][0], 0x03);
        // 57 bytes = 19 LEDs completos: a segunda parte também começa em G.
        assert_eq!(&reports[1][1..4], &[0x00, 0x7f, 0xff]);
        let breathing = ChannelLighting {
            mode: Mode::Breathing,
            ..Default::default()
        };
        assert_eq!(smart_lighting(&breathing).unwrap()[0][2], 0x07);
        assert!(reports.iter().all(|r| r.len() == REPORT_LEN));
    }

    #[test]
    fn smart_init_sequence() {
        let [detect, start] = smart_init();
        assert_eq!(&detect[..2], &[0x01, 0x5c]);
        assert_eq!(&start[..2], &[0x01, 0x5d]);
        assert_eq!(start.len(), REPORT_LEN);
    }
}
