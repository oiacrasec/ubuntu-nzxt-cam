#!/usr/bin/env bash
set -euo pipefail
PROJECT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
INSTALL_PREFIX="${HOME}/.local"
AUTOSTART=false
BUILD=true
while (($#)); do
    case "$1" in
        --prefix) INSTALL_PREFIX="${2:?Informe o prefixo}"; shift 2 ;;
        --autostart) AUTOSTART=true; shift ;;
        --no-build) BUILD=false; shift ;;
        -h|--help) echo 'Uso: ./install-desktop-app.sh [--prefix DIRETÓRIO] [--autostart] [--no-build]'; exit 0 ;;
        *) echo "Opção desconhecida: $1" >&2; exit 1 ;;
    esac
done
if [[ "$INSTALL_PREFIX" != /* || "$INSTALL_PREFIX" == / ]]; then
    echo 'Use um prefixo absoluto dedicado, como ~/.local.' >&2; exit 1
fi
cd -- "$PROJECT_DIR"
if "$BUILD"; then
    CARGO="$(command -v cargo || true)"
    [[ -n "$CARGO" ]] || CARGO="$HOME/.cargo/bin/cargo"
    [[ -x "$CARGO" ]] || { echo 'Instale Rust 1.95 ou posterior.' >&2; exit 1; }
    CARGO_INCREMENTAL=0 "$CARGO" build --release --locked
fi
TARGET_DIR="${CARGO_TARGET_DIR:-$PROJECT_DIR/target}"
BINARY="$TARGET_DIR/release/nzxt-cam"
[[ -x "$BINARY" ]] || { echo 'Binário ausente. Execute cargo build --release --locked.' >&2; exit 1; }

BIN_DIR="$INSTALL_PREFIX/bin"
APP_DIR="$INSTALL_PREFIX/share/applications"
ICON_DIR="$INSTALL_PREFIX/share/icons/hicolor/scalable/apps"
mkdir -p -- "$BIN_DIR" "$APP_DIR" "$ICON_DIR"
# Troca atômica: o processo em execução conserva o binário anterior até reiniciar.
install -m 0755 -- "$BINARY" "$BIN_DIR/.nzxt-cam.new"
mv -f -- "$BIN_DIR/.nzxt-cam.new" "$BIN_DIR/nzxt-cam"
install -m 0644 -- assets/nzxt-cam.svg "$ICON_DIR/nzxt-cam.svg"
EXEC="${BIN_DIR// /\\s}/nzxt-cam"
cat > "$APP_DIR/.nzxt-cam.desktop.new" <<EOF
[Desktop Entry]
Type=Application
Name=NZXT CAM
Comment=Refrigeração, iluminação e monitoramento
Comment[en_US]=Cooling, lighting and monitoring
Exec=$EXEC
Icon=nzxt-cam
Terminal=false
Categories=System;Monitor;HardwareSettings;
StartupWMClass=nzxt-cam
EOF
mv -f -- "$APP_DIR/.nzxt-cam.desktop.new" "$APP_DIR/nzxt-cam.desktop"
"$BIN_DIR/nzxt-cam" --register-installation "$INSTALL_PREFIX"
if "$AUTOSTART"; then "$BIN_DIR/nzxt-cam" --install-autostart; fi
echo "Instalado: $BIN_DIR/nzxt-cam"
echo 'Abra NZXT CAM no menu de aplicativos.'
echo 'Para acessar os dispositivos sem root: Configurações › Acesso aos dispositivos › Preparar permissões…'
