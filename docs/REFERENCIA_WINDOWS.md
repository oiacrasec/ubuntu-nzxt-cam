# NZXT CAM Windows como referência de migração

O NZXT CAM 4.76.8.70 instalado na máquina Windows do usuário é a referência de
paridade. Esta página registra a identidade dos arquivos, a divisão entre interface
e núcleo nativo, os dispositivos do usuário e os recursos observados.
Análise estática de 12/11/2026; nenhum componente do CAM foi executado para ela.

## Artefatos

A instalação em `C:\Program Files\NZXT CAM` foi copiada para
`package/NZXT CAM 4.76.8/`: 415 arquivos, 522.790.607 bytes. O instalador não foi
usado; a identidade da referência é a instalação. Se o setup for anexado depois,
registrar aqui seu SHA-256.

| Arquivo | Bytes | SHA-256 |
| --- | --- | --- |
| `NZXT CAM.exe` (4.76.8.70, NZXT, Inc.) | 186.398.128 | `ae0130025675a6ce4a8e6c91d66121e57e10eb884d52b3a36448a467c319b16b` |
| `resources/app.asar` | 150.365.897 | `2c6e2e16b98d53e65d6bb69bae7fd91d867635bb40b386588077fc46de797b1c` |
| `@nzxt/cam-core/dist/index.node` | 48.624.048 | `b8e82a10749ce330aa4276888f50762e9c6008471f21e6532dceb34b559dca3b` |
| `@nzxt/cam-core/dist/service.exe` | 567.728 | `009ff5884f4e4e0f5655060dd670358376ee4f82bf3bf4855b9d5f21ec95265a` |
| `@nzxt/cam-core/dist/cam_helper.exe` | 4.790.704 | `c36b97dce72ae66c2f309e3486f371579ff64143071031efdadf0785e6fa6c13` |

Os caminhos de `@nzxt/cam-core` ficam em `resources/app.asar.unpacked/node_modules/`.

## Ferramentas

- REA 6.7.0 (checkout local em `D:\Projetos\PycharmProjects\rea`, CLI), com os
  arquivos nativos Windows do pacote oficial `rea-agents@6.7.0`; o SHA-256 de
  `rea-windows-x64.node` confere com o manifesto do pacote.
- Ghidra 12.1.4 da [release oficial](https://github.com/NationalSecurityAgency/ghidra/releases/tag/Ghidra_12.1.4_build),
  ZIP com SHA-256 `ddac49f903da9d5bac833e5cc79395098b9c33cfd3279be5f31bd00387d2d4db`
  conferido contra o digest publicado; JDK Temurin 25.0.3. `rea doctor` aprovou
  versão, headless, decompilador `win_x86_64`, Java e controles nativos.
- Leitura do índice do `app.asar` e dos catálogos de tradução por script próprio,
  sem extrair o pacote.

## Divisão do CAM

| Componente | Papel observado | Consequência para o porte |
| --- | --- | --- |
| Electron (`NZXT CAM.exe`, `app.asar`) | Interface React/MobX, configurações, perfis, Modo Mini | Funções e layout reproduzidos em egui; código não reutilizado |
| `@nzxt/cam-core/dist/index.node` | Núcleo nativo; strings e nomes indicam Rust | Fonte principal de protocolo dos dispositivos |
| `service.exe`, `cam_helper.exe`, drivers `.sys` | Serviço e acesso privilegiado a sensores e placa-mãe | Substituídos por hwmon, NVML e regra udev |
| `cpuidsdk64.dll` | Sensores de CPU/placa (SDK CPUID) | Substituído por hwmon do kernel |
| `firmware-update.exe`, `rvclib-fw-updater.exe` | Atualização de firmware | Fora do escopo |
| `dts_cmedia_driver`, `clear-voice-apo` | Áudio de produtos NZXT | Fora do escopo atual |

## Dispositivos do usuário

Identificados no Windows por VID/PID em 12/11/2026. O Windows mostra apenas
"NZXT USB Device"; o modelo vem da correspondência do PID.

| PID | Modelo | Evidência no CAM |
| --- | --- | --- |
| `1e71:170e` | Kraken X52/X62/X72 | `index.node` contém `KrakenX` e as famílias de firmware `KrakenX526272_v5` e `KrakenX526272_v6`; o catálogo de nomes tem `device-KrakenX` |
| `1e71:1714` | Smart Device (V1) | `index.node` contém `SmartDevice`, distinto de `SmartDevice2`/`SmartDevice3`; o renderer lista `NoiseDeviceType.SmartDevice` |

A tabela PID → família dentro do `index.node` ainda não foi localizada; a
correspondência acima é inferência a confirmar no Ghidra.

## Recursos observados na interface

Extraídos dos catálogos `resources/static/translations/en/*.json` e do renderer.
Presença de texto não comprova que o recurso se aplica ao hardware do usuário.

| Área | Itens observados |
| --- | --- |
| Refrigeração | Perfis `Silent`, `Performance`, `Fixed`, `Custom`; fonte de temperatura CPU, GPU, Liquid ou PSU; seleção de GPU; aplicar curva a outros canais; canais `Pump` e `Fan`; tipo DC/PWM; "Case Noise"; perfis salvos com nome |
| Iluminação | Canais (`Ring`, `Logo`, `Channel 1..6`), contagem de LEDs; modos estáticos `Fixed`, `Breathing`, `Fading`, `Marquee`, `Covering Marquee`, `Pulse`, `Spectrum Wave`, `Alternating`, `Starry Night`, `Rainbow Flow`, `Super Rainbow`, `Rainbow Pulse`, `Candle`, `Wave`, `Tai Chi`, `Water Cooler`, `Loading`; modos dinâmicos `Temperature Monitor`, `Audio Level`, `Audio Bass`; opções velocidade (cinco níveis), direção, tamanho de LED, movimento |
| Monitoramento | CPU (carga, clock, temperatura, fan), GPU (carga, clock, temperatura, fan), RAM, armazenamento e rede; temperatura da CPU por pacote ou média dos núcleos |
| Perfis | Profile Sync: perfis mestres que ativam vários perfis de dispositivo, com auto-launch por programa ou por horário |
| Aplicação | Iniciar com o sistema, iniciar minimizado, Modo Mini (sempre no topo, encaixe na borda, painéis), °C/°F, bandeja |
| Estados de dispositivo | Inicializando, pronto, falha de comunicação, conflito com outro programa, aguardando retomada |

## Lacunas da análise

- Protocolo HID do Kraken X e do Smart Device V1: relatórios, curvas, modos de
  iluminação e leitura de status ainda não foram levantados no `index.node`.
- Quais modos de iluminação o CAM habilita para cada dispositivo do usuário.
- Comportamento do CAM ao fechar: o que fica gravado no dispositivo.

A documentação pública do liquidctl indica que o Kraken X52/X62/X72 aceita perfis
de bomba e fan baseados na temperatura do líquido e que o Smart Device V1 aceita
só rotação fixa por canal. Isso orienta a investigação; não substitui a evidência
do CAM nem um ensaio no Ubuntu.
