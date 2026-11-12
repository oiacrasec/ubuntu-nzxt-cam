# Desenvolvimento

**Estado: Proposto.** Ainda não há `Cargo.toml`; esta página fixa o ciclo previsto.
Compile e teste no Ubuntu 24.04+ de destino.

## Preparação prevista

```bash
sudo apt install build-essential pkg-config libxkbcommon-dev libwayland-dev \
  libx11-dev libgl1-mesa-dev
export PATH="$HOME/.cargo/bin:$PATH"
cargo build --release --locked
```

Rust 1.95 ou posterior ([instalação](https://rustup.rs/)). `cargo run --locked`
abrirá a janela; `cargo run --locked -- --headless`, somente o controlador.

## Camadas

| Camada | Módulos | Contrato |
| --- | --- | --- |
| Dados e limites | `config`, `curves`, `lighting`, `devices` | Tipos, validação e descoberta |
| Execução | `sensors`, `governor`, `kraken_x`, `smart_device` | Leitura e escrita, sem dependência da janela |
| Aplicação | `engine`, `profiles` | Comandos, transições, persistência e snapshot |
| Adaptadores | `ipc`, `main`, `gui`, `tray`, `desktop` | Transporte, interação e sessão |

Um único crate com módulos, como no Facecam Hub. A thread gráfica não espera
dispositivo nem sensor.

## Instância de teste

Use um `XDG_RUNTIME_DIR` e um `--config` temporários para uma instância de teste,
sem encerrar o controlador de uso diário. Não aponte testes para dispositivos reais.
