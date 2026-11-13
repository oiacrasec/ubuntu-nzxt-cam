#!/usr/bin/env bash
set -euo pipefail
INSTALL_PREFIX="${HOME}/.local"
PURGE=false
while (($#)); do
    case "$1" in
        --prefix) INSTALL_PREFIX="${2:?Informe o prefixo}"; shift 2 ;;
        --purge) PURGE=true; shift ;;
        -h|--help) echo 'Uso: ./uninstall-desktop-app.sh [--prefix DIRETÓRIO] [--purge]'; exit 0 ;;
        *) echo "Opção desconhecida: $1" >&2; exit 1 ;;
    esac
done
if [[ "$INSTALL_PREFIX" != /* || "$INSTALL_PREFIX" == / ]]; then
    echo 'Use um prefixo absoluto dedicado, como ~/.local.' >&2; exit 1
fi
BINARY="$INSTALL_PREFIX/bin/nzxt-cam"
if [[ -x "$BINARY" ]]; then
    "$BINARY" --quit >/dev/null 2>&1 || true
    "$BINARY" --remove-autostart || true
fi
rm -f -- "$BINARY" \
    "$INSTALL_PREFIX/bin/nzxt-cam.installation.json" \
    "$INSTALL_PREFIX/share/applications/nzxt-cam.desktop" \
    "$INSTALL_PREFIX/share/icons/hicolor/scalable/apps/nzxt-cam.svg" \
    "$INSTALL_PREFIX/share/icons/hicolor/512x512/apps/nzxt-cam.png"
CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/nzxt-cam"
if "$PURGE"; then
    rm -rf -- "$CONFIG_DIR"
    echo "Configuração removida: $CONFIG_DIR"
else
    echo "Configuração preservada: $CONFIG_DIR (use --purge para remover)"
fi
if [[ -f /etc/udev/rules.d/71-nzxt-cam.rules ]]; then
    echo 'A regra udev continua instalada. Antes de desinstalar, remova-a no aplicativo'
    echo '(Configurações › Acesso aos dispositivos › Remover regra…) ou com:'
    echo '  sudo rm /etc/udev/rules.d/71-nzxt-cam.rules && sudo udevadm control --reload-rules'
fi
echo 'NZXT CAM removido.'
