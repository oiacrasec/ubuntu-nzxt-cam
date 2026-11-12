# Uso do NZXT CAM para Ubuntu

Compile, instale, prepare as permissões e abra o aplicativo pelo menu.
Esta página descreve o comportamento implementado; o estado de validação de cada
item está no [roadmap](ROADMAP.md).

## Compilar e instalar

```bash
sudo apt install build-essential pkg-config libxkbcommon-dev libwayland-dev \
  libx11-dev libgl1-mesa-dev libdbus-1-dev
export PATH="$HOME/.cargo/bin:$PATH"
./test-desktop-app.sh
./install-desktop-app.sh            # ~/.local por padrão
./install-desktop-app.sh --autostart  # opcional: iniciar com a sessão
```

Use Rust 1.95 ou posterior ([instalação](https://rustup.rs/)). O instalador copia
`nzxt-cam` para `~/.local/bin`, cria o atalho do menu e o ícone. `./uninstall-desktop-app.sh`
remove o aplicativo e preserva a configuração; `--purge` também apaga a configuração.

## Preparar o acesso aos dispositivos

Sem preparação, `/dev/hidraw*` pertence ao root e o dispositivo aparece como
**Sem permissão de acesso**.

1. Abra **Configurações** › **Acesso aos dispositivos** › **Preparar permissões…**.
2. Revise os comandos e o conteúdo da regra exibidos. Nada foi alterado ainda.
3. Clique em **Confirmar e autenticar** e informe a senha na janela do Ubuntu.
4. Ao terminar, o aplicativo reabre os dispositivos; o estado passa a **Pronto**.

A regra `/etc/udev/rules.d/71-nzxt-cam.rules` concede à sessão ativa somente o
Kraken X (`1e71:170e`) e o Smart Device V1 (`1e71:1714`). **Remover regra…** desfaz.
Pela CLI: `nzxt-cam --prepare-permissions`, depois `nzxt-cam --confirm-plan ID`.

## Telas

- **Monitoramento**: CPU, GPU, RAM, rede, armazenamento e, por dispositivo, temperatura
  do líquido, RPM da bomba e dos fans e ruído do gabinete.
- **Refrigeração**: por canal, perfis **Silent**, **Performance**, **Fixed** e **Custom**,
  fonte **Líquido**, **CPU** ou **GPU**, editor de curva e **Aplicar curva a outros canais**.
- **Iluminação**: modo, cores, velocidade e direção conforme o que o canal aceita.
- **Configurações**: idioma, °C/°F, temperatura da CPU por pacote ou média dos núcleos,
  início com a sessão, início na bandeja, Modo Mini e acesso aos dispositivos.

O botão **▣** no topo abre o **Modo Mini**, uma janela compacta, opcionalmente
sempre no topo. Com a bandeja disponível, fechar a janela mantém o governador
ativo; **Sair** na bandeja encerra o aplicativo. No GNOME, a bandeja pode exigir a
extensão AppIndicator.

## Segurança térmica

- Toda curva precisa chegar a 100% na temperatura crítica da fonte: 60 °C para o
  líquido, 90 °C para CPU e GPU. A interface recusa curvas que não cheguem.
- Uma fonte indisponível leva o canal a 100% e mostra **Segurança** no canal.
- No Kraken X, a curva com fonte **Líquido** roda no próprio dispositivo. Com fonte
  CPU ou GPU, o aplicativo grava uma curva plana que ainda sobe a 100% com o
  líquido a 60 °C, mesmo que o aplicativo pare.
- O Smart Device V1 não tem curva interna. Ao sair normalmente, os fans vão para a
  rotação de saída (padrão 100%). Se o processo morrer abruptamente, fica a última
  rotação gravada: é uma limitação conhecida.

## Linha de comando

```bash
nzxt-cam --list-devices      # descoberta, sem abrir dispositivos
nzxt-cam --sensors           # uma leitura de sensores em JSON
nzxt-cam --headless          # controlador sem janela
nzxt-cam --status            # estado da instância em execução
nzxt-cam --set-cooling 'kraken_x:SERIAL/pump' perfil.json
nzxt-cam --set-lighting 'kraken_x:SERIAL/ring' luz.json
nzxt-cam --quit
```

O identificador do canal aparece em `--status`, em `devices[].info.id`, seguido de
`/` e do nome do canal (`pump`, `fan`, `fan1`…). Exemplos de arquivo:

```json
{"profile": {"type": "custom", "points": [
  {"temperature": 30, "duty": 40}, {"temperature": 45, "duty": 70},
  {"temperature": 60, "duty": 100}]}, "source": "liquid"}
```

```json
{"mode": "breathing", "colors": [[126, 34, 206]], "speed": "slow"}
```

## Configuração

O arquivo fica em `~/.config/nzxt-cam/config.json`, respeitando `XDG_CONFIG_HOME`;
`--config ARQUIVO` seleciona outro. Campos desconhecidos na raiz são preservados,
um JSON inválido permanece intacto e o primeiro salvamento guarda
`config.json.backup`. Traduções da comunidade podem ser colocadas em
`~/.config/nzxt-cam/locales/<idioma>.json`, com as mesmas chaves de
[`locales/en-US.json`](../locales/en-US.json).
