# NZXT CAM Windows como referência de migração

O NZXT CAM 4.76.8.70 instalado na máquina Windows do usuário é a referência de
paridade. Esta página registra a identidade dos arquivos, a divisão entre interface
e núcleo nativo, os dispositivos do usuário e os recursos observados.
Análise estática de 12/11/2026; nenhum componente do CAM foi executado para ela.
O diagnóstico de conflitos abaixo é uma observação separada do CAM já em execução,
por consultas de leitura no Windows e nos registros locais.

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

## Possíveis conflitos com outros softwares

Outro programa de RGB ou monitoramento pode disputar o acesso ao dispositivo e
impedir que o CAM aplique comandos. A presença de programas simultâneos, sozinha,
não comprova conflito nem identifica o responsável.

A [orientação oficial da NZXT](https://support.nzxt.com/hc/en-us/articles/48228772206107-My-NZXT-device-is-not-responding-to-CAM)
descreve interrupções do acesso exclusivo por outros programas e recomenda
identificar os processos que mantêm o dispositivo aberto.

**Evidência da referência Windows, coletada em 12/11/2026.** Ambiente: NZXT CAM
4.76.8, Windows build 26300, Kraken X (`1e71:170e`) e Smart Device V1
(`1e71:1714`). Procedimento: consultar processos, serviços, configuração de
iluminação do Windows, dispositivos presentes e arquivos de log, sem alterar
configurações, encerrar processos ou enviar comandos aos dispositivos.

| Observação | Resultado e limite da evidência |
| --- | --- |
| Programas ativos durante a consulta | CAM, Corsair iCUE 5.52.93 e componentes ASUS Aura/Armoury; serviço `LightingService` ativo e plugin ASUS instalado no iCUE. Nenhum deles foi confirmado como responsável pela falha. |
| Iluminação Dinâmica do Windows | Habilitada (`AmbientLightingEnabled = 1`); não foi verificado se controla alguma das peças afetadas. |
| Registro de 03/11/2026 | O `cam.log` registra `opened with shared access` para Kraken X e Smart Device, na sessão do CAM 4.76.6. É um indício de ausência de exclusividade, sem identificar outro processo. |
| Registro de 07/11/2026 | Comandos de duty dos canais de fan e bomba do Kraken falharam com `Fwk1SetRpm`, causado por `CallbackTimeout`. O timeout confirma falha de comando, mas não prova conflito com outro software. |
| Sessão consultada em 12/11/2026 | Ambos os dispositivos estavam com status `OK` no Windows e foram inicializados pelo CAM 4.76.8; não houve aviso de acesso compartilhado nessa inicialização. Isso não exclui falhas intermitentes. |

Os registros consultados ficam em `%APPDATA%\NZXT CAM\logs\cam.log`.
Não foi reproduzida a falha durante a investigação, não houve teste de isolamento
dos programas e não foi identificado qual processo concorrente abriu o dispositivo.
**Não verificado:** relação causal entre os programas listados e os sintomas do usuário.

Para confirmar uma disputa de acesso no Windows, durante a falha:

1. No **Gerenciador de Dispositivos**, abra as propriedades do dispositivo NZXT.
   Se necessário, use **Exibir** › **Dispositivos por contêiner** e selecione a
   interface HID correspondente.
2. Em **Detalhes**, copie o **Nome do objeto de dispositivo físico** (PDO).
   Obtenha o valor na sessão atual; ele pode mudar após reiniciar.
3. No [Process Explorer da Microsoft](https://learn.microsoft.com/en-us/sysinternals/downloads/process-explorer),
   use **Find** › **Find Handle or DLL** para pesquisar esse PDO.
4. Registre os processos encontrados, o dispositivo, o horário e o sintoma.
   Outro programa com um handle aberto é evidência de acesso concorrente;
   atribuir a falha a ele ainda exige correlacionar o acesso com o sintoma.
   Encontrar apenas componentes do CAM não descarta um conflito transitório anterior.

Um teste de isolamento que envolva encerrar controladores de refrigeração exige
verificar a segurança térmica antes; não integra os checks comuns nem foi feito
nesta investigação. As evidências acima se aplicam ao CAM Windows. O contrato
de acesso do porte está em [Arquitetura](ARQUITETURA.md#acesso-aos-dispositivos);
esta investigação não valida a detecção de conflitos no Ubuntu.

## Detalhe da iluminação observado no CAM

**Evidência da referência Windows, coletada em 12/11/2026.** CAM 4.76.8,
Kraken X e Smart Device V1. Com autorização explícita do usuário, foram feitas
duas mudanças temporárias de iluminação pela interface do CAM; não houve
alteração de refrigeração nem firmware.

- No Anel em **Fixo**, ligar **Personalizar cores LED** substitui o chip de cor
  pela instrução **Seleciona qualquer LED abaixo**. Cada um dos oito segmentos
  da prévia é clicável e abre o seletor **COLOR SWATCHES**, com **Custom** e
  **Cancelar**. Os segmentos apareceram brancos nessa sessão; não foi verificado
  se essa cor é um padrão ou uma configuração por LED já persistida.
- Na Fita em **Respirar**, as cores aparecem como amostras sem hexadecimal,
  com **Add Color** ao lado. A velocidade fica abaixo da prévia, em um único
  slider; o valor observado foi **Velocidade Normal**. Não foram mudadas as
  cores, a velocidade nem o modo da Fita.
- No Logótipo, selecionar **Temperatura do CPU** substitui a prévia por uma
  lista de limites e cores, com o rótulo **é superior a**. Nessa sessão havia
  sete linhas: 0 °C → `#00FF00`, 65 °C → `#84FF00`, 70 °C → `#BAFF00`,
  75 °C → `#FFFF00`, 80 °C → `#FFAE00`, 85 °C → `#FF4800` e
  90 °C → `#FF0000`. Há controles de remover linhas e de adicionar uma linha.
  Os limites não foram editados; interpolação, comportamento na igualdade,
  unidade após trocar °C/°F e modo GPU não foram verificados.

Ao terminar, foi confirmado visualmente no CAM: Logótipo e Anel em **Fixo**
`#51007A`, personalização do Anel desligada e Fita em **Respirar**.
Isso confirma a restauração na interface; não foi feita medição física dos LEDs.

Capturas e utilitário de observação ficam em `package/`, fora do Git:
`cam-ring-per-led-on.png`, `cam-ring-led-selected.png`,
`cam-logo-cpu-temperature.png`, `cam-lighting-restored.png` e `cam-observe.ps1`.

## Menus e opções de cada modo (13/11/2026)

A pedido do usuário, cada modo do Anel e da Fita foi escolhido no CAM 4.76.8 e o
estado original foi restaurado no fim; o arquivo
`%APPDATA%\NZXT CAM\DataStorage\latest\user\lightingController.json` ficou igual
ao de antes. O `renderer.log` do CAM registra em `client.modifyChannel` os campos
de cada modo escolhido. Esses campos são as opções que o painel mostra.

| Canal | Menu, na ordem do CAM |
| --- | --- |
| Logótipo | Fixo, Respirar, Desvanecer, Pulsar, Onda de espectro, Temperatura do CPU, Temperatura da GPU, Nivelado, Sincronização, Graves |
| Anel | Fixo, Respirar, Desvanecer, Painel, Painel rolante, Pulsar, Onda de espectro, Alternado, Asas, Tai Chi, Refrigerador de água, A carregar, Temperatura do CPU, Temperatura da GPU, Nivelado, Sincronização, Graves |
| Fita | Fixo, Respirar, Desvanecer, Painel, Painel rolante, Pulsar, Onda de espectro, Alternado, Vela, Asas, Temperatura do CPU, Temperatura da GPU, Onda, Nivelado, Sincronização, Graves, Ganho |

| Modo | Opções e cores padrão |
| --- | --- |
| Fixo | uma cor `#51007A`; **Personalizar cores LED** no Anel e na Fita |
| Respirar | velocidade; 1 a 8 cores; **Personalizar cores LED** no Anel e na Fita |
| Desvanecer | velocidade; cores `#51007A`, `#00FF00`, `#0000FF` |
| Painel | velocidade, direção, tamanho do LED (3 a 6); uma cor |
| Painel rolante | velocidade, direção; cores `#0000FF`, `#00FF00` |
| Pulsar | velocidade; 1 a 8 cores |
| Onda de espectro | velocidade, direção; **Cores: Automático** |
| Alternado | velocidade, direção, tamanho do LED, **Movimento**; cores `#FF0000`, `#0000FF` |
| Asas | velocidade; uma cor `#0000FF` |
| Tai Chi | velocidade; cores `#FF0000`, `#0000FF` |
| Refrigerador de água | velocidade; **Cores: Automático** |
| A carregar | uma cor, sem velocidade |
| Vela | uma cor `#FFA500`, sem velocidade |
| Onda (Fita) | só a edição por LED (**Seleciona qualquer LED abaixo**) |
| Nivelado, Sincronização, Ganho | lista numerada de 8 cores, sem prévia |
| Graves | lista numerada com uma cor |

O brilho do canal fica desativado em Onda de espectro, Refrigerador de água,
temperatura e áudio. Com uma cor, o painel mostra o botão com o hexadecimal;
com várias, amostras e **Add Color**. Velocidade, direção, tamanho do LED e
movimento ficam numa linha abaixo da prévia.

### Captura USB dos modos de áudio (13/11/2026)

Os modos de áudio são calculados pelo núcleo nativo do CAM (`cam-core`), sem
algoritmo legível. Para ver o comportamento, o tráfego USB foi gravado com
USBPcap 1.5.4 enquanto o CAM tocava música. O CAM tinha se atualizado sozinho
para a **4.76.9.72**, então a captura vale para essa versão. O estado de
iluminação foi restaurado depois, pelo arquivo `lightingController.json` salvo
antes, com o CAM fechado.

| Modo | O que o CAM enviou |
| --- | --- |
| Nivelado (Anel) | `02 4c 02`, modo Fixo, cerca de 20 quadros por segundo. Medidor: os N primeiros LEDs acesos, o LED *i* com a cor *i* da lista, sem reduzir o brilho; os demais apagados (`1 2 3 4 5 6 . .`) |
| Sincronização (Anel) | O mesmo medidor, mais suave: muda no máximo 2 passos para cima e 3 para baixo por quadro (Nivelado chega a 4) |
| Graves (Anel e Logótipo) | Todos os LEDs com `#51007A` multiplicada pela intensidade dos graves (até cerca de 75%) |
| Nivelado e Sincronização (Logótipo) | Uma cor da lista, com brilho total: a do último passo aceso do medidor de 8 passos |
| Onda (Fita) | Um único `02 4b` com modo `0x00` e todos os LEDs apagados |
| Nivelado, Sincronização, Graves e Ganho (Fita) | Nenhum relatório de iluminação |

Na Fita do Smart Device V1, portanto, o CAM 4.76.9 apaga o canal em Onda e não
anima os modos de áudio. O porte mantém esses modos funcionando: os de áudio
usam o mesmo medidor sobre os 40 LEDs do canal, e Onda envia o modo `0x0d`
(`super-wave` nos fatos de protocolo do liquidctl), ainda sem confirmação no
hardware. O algoritmo de Ganho é próprio do porte.

### Outros programas e refrigeração observados na captura

O usuário relatou que os efeitos não aparecem nos LEDs como deveriam e suspeitou
de conflito com outro software. Nesta máquina rodam ASUS Aura/Armoury Crate
(`LightingService`, `AacAmbientLighting`), Corsair iCUE 5 (com
`CorsairCpuIdService`), Creative Sound Blaster Command e Wallpaper Engine. A
CPUID SDK (`cpuidsdk64.dll`) do iCUE e do `AsusFanControlService` cita
"NZXT Kraken", e o Armoury Crate lista `1E71/170E` e `1E71/1714` só no
inventário de diagnóstico. Nas duas capturas (6,5 min), **todos** os relatórios
enviados ao Kraken e ao Smart Device eram do CAM; nenhum outro programa escreveu
neles. O que não aparece nos LEDs se explica pelo próprio CAM: na Fita ele não
anima os modos de áudio e apaga em Onda, e em Graves o Anel não passa de 75% de
`#51007A`, um roxo escuro. Leitura de sensores por terceiros (como a CPUID SDK)
não aparece como escrita e não foi descartada.

O usuário relatou que a logo do Kraken às vezes acende ao ligar o PC e depois
apaga. As capturas começaram com o PC ligado havia minutos, então não descartam
uma escrita feita só na inicialização de outro programa. Suspeitos: iCUE e
`CorsairCpuIdService`, ASUS `LightingService`/`AacAmbientLighting` e o próprio
CAM, cujo perfil ativo tem duas identidades para o mesmo Kraken
(`kraken_x-227` com logo `#FFFFFF` e `kraken_x-107544401-…` com `#A900FF`).
Teste pendente (exige reinstalar o USBPcap e reiniciar o Windows): capturar,
pôr a logo em Onda de espectro e reiniciar um suspeito por vez, observando a
logo e os relatórios enviados ao Kraken. No Ubuntu, o porte detecta outro
processo com o hidraw aberto (`devices::holders`) e mostra o conflito.

Atualização de 13/11/2026: no Ubuntu, sem nenhum desses programas, anel e logo
também ignoraram todos os comandos. Depois de 1 min com a fonte desligada, ficaram
apagados desde o boot, antes de qualquer software, e assim continuaram com
qualquer cor ou modo. Como o sintoma ocorre sem nenhum desses programas, ele não
vem de conflito com outros softwares.

Atualização de 14/11/2026: depois de reiniciar o Windows, anel e logo estavam
acesos e o Anel respondeu à troca para **Onda de espectro** no CAM. O sintoma é
intermitente e a causa segue desconhecida; a hipótese de defeito permanente foi
descartada ([Validação local](VALIDACAO_LOCAL.md#iluminação-do-kraken-x--13112026)).

A cada segundo, o CAM 4.76.9 envia ao Kraken uma rajada de 36 relatórios
`02 4d`: curva do fan com 17 pontos de 20 °C a 100 °C, de 5 em 5 °C
(`80 14 1e` … `90 64 64`), rotação fixa do fan (`00 00 21`), curva da bomba com
17 pontos (`c0 14 3c` … `d0 64 64`) e rotação fixa da bomba (`40 00 42`). O porte
grava a curva interna de 36 pontos de 20 °C a 60 °C só quando ela muda. A
diferença afeta refrigeração e fica para revisão no Ubuntu, com o hardware.
Script: `package/usb-captura/rajada.py`.

Arquivos em `package/usb-captura/`: as capturas `captura1-logo-graves.pcap` e
`captura2-anel.pcap`, os roteiros de captura e de varredura e os scripts de
análise `ler_pcap.py`, `kraken.py` e `anel2.py`.
Folhas de contato com os painéis: `package/cam-efeitos-anel.png` e
`package/cam-efeitos-fita.png`.
O utilitário captura somente a janela do CAM, valida as coordenadas e restaura
a posição do cursor após cada ação. As capturas não são recursos do porte.

**Implementado no porte, sem validação no Ubuntu:** slider de velocidade com
cinco níveis, amostras de cor com botão de adicionar e seleção de LED na prévia
para abrir o seletor de cor. O seletor e os desenhos são próprios em egui;
não foram copiados recursos do CAM. **Proposto:** modos por temperatura e
seu editor de limites; a observação da interface não confirma sua implementação.

**Implementado em 13/11/2026, sem validação no Ubuntu:** modos **Temperatura do CPU**
e **Temperatura da GPU** para Logótipo, Anel e Fita, com os limites padrão acima,
editor `é superior a` (adicionar, remover, até 10 limites crescentes entre 0 e 120 °C)
e cor da faixa enviada como **Fixo** pelo controlador quando a faixa muda. Escolhas
não confirmadas no CAM: troca por faixa sem interpolação; na igualdade vale a faixa
anterior; sem leitura do sensor, a última cor enviada é mantida. Unidade sempre °C.

## Revisão lado a lado de monitoramento, especificações e Profile Sync

**Evidência da referência Windows, coletada em 12/11/2026, CAM 4.76.8.** Foram
abertos os detalhes de CPU, GPU, placa-mãe, quatro módulos RAM, três discos, dez
itens de áudio e o adaptador de rede visível. O procedimento só navegou pela
interface; não mudou perfis, iluminação, refrigeração nem firmware.

| Tela ou item | Campos/controles observados |
| --- | --- |
| Principais processos | Nome, CPU, GPU, RAM, Baixar e Fazer upload; seta de ordenação junto ao cabeçalho selecionado |
| Rede (monitoramento) | Tráfego de envio e recebimento, com setas para cima e para baixo |
| CPU | Fabricante, nome do código, clock padrão e atual, tipo de soquete, energia e TDP |
| GPU | Modelo, clock atual e energia |
| Placa-mãe | Modelo, chipset, southbridge, versão e data do BIOS |
| Cada módulo RAM | Modelo, capacidade, frequência padrão, tipo e tempos |
| Cada disco | Nome, fabricante, formato e capacidade |
| Cada item de áudio | Nome, formato e número de canais; há valores N/A conforme o dispositivo |
| Rede (especificações) | Nome do adaptador, tipo de conexão e endereço IP |
| Profile Sync | Seletor de perfil mestre, Add Profile, Auto Launch desabilitado para Default e seleção separada dos perfis de iluminação/refrigeração |

Capturas locais em `package/cam-monitor-review.png`, `package/cam-spec-*.png` e
`package/cam-profile-menu.png`. O endereço IP observado não foi transcrito aqui.
Esses campos não comprovam a disponibilidade de todas as leituras no Ubuntu.
O porte mostra `—` para fontes ainda indisponíveis; a prévia usa exemplos simulados.

Para **Personalizar cores LED** da Fita em **Respirar**, a
[documentação pública do Smart Device V1](https://github.com/liquidctl/liquidctl/blob/main/docs/nzxt-smart-device-v1-guide.md#rgb-lighting)
descreve `super-breathing`: até 40 cores, uma por LED, em um único passo.
O porte usa o formato de relatório já confirmado no CAM (`02 4b` + `03`), com o
mesmo id de Respirar `0x07` e passo zero, preenchendo o vetor GRB por LED. Não foi
copiado código do liquidctl nem criado novo comando. O layout foi testado com
transporte simulado; aplicação física desse modo ainda não foi validada no Ubuntu.

## Inspeção detalhada de Profile Sync — 13/11/2026

**Evidência da referência Windows, CAM 4.76.8.** Com autorização expressa, foi
criado e ativado `Inspecao temporaria Codex`, permitindo abrir as opções que
ficam desabilitadas em Default. Foram observados:

- Página com seletor mestre, ajuda, Auto Launch e Selected Profiles; sem
  formulários inline de captura nem botão Salvar e aplicar.
- Add/Edit Profile Sync: nome (limite de 50 caracteres), Salvar, Cancelar ou
  Excluir, e fechamento pelo X. Criar ativa imediatamente o novo mestre.
- With Program: pesquisa, Connect/Remover, lista de programas conectados,
  Browse Programs (seletor nativo de executáveis), Salvar e Cancelar.
- At Specific Time: sete dias, início, fim, All Day e +1 Day. Sem dias,
  a borda fica vermelha e Salvar é desabilitado. A engrenagem reabre a regra.
- Setas de Selected Profiles navegam para a página do dispositivo. Adicionar
  perfis nessas páginas oferece nome e Duplicate existing profile.

As condições por programa e horário e a pilha transitória de ativações foram
também conferidas por análise estática do bundle original, sem copiar código.
Capturas locais: `package/cam-sync-*.png`, `cam-light-profile-*.png` e
`cam-cooling-add-dialog.png`. A pesquisa e conexão foram canceladas; uma regra
temporária de sábado foi salva e depois removida.

**Restauração confirmada na interface:** perfil temporário excluído, somente
Default no seletor, iluminação Caio e refrigeração Padrão. Logótipo Fixo
`#51007A`, Anel Fixo e Fita Respirar. Evidências em
`package/cam-sync-deletion-verified.png` e `cam-light-restored-logo.png`.

## Logo do cabeçalho — 13/11/2026

As capturas do CAM 4.76.8, incluindo `package/cam-sync-restored.png`, mostram
a logo NZXT CAM no canto superior esquerdo. O recurso correspondente está em
`assets/originals/logo nzxt cam.svg` no `app.asar`, com proporção 218 × 36.
Por pedido expresso do usuário, foi incluído no porte: SVG original e PNG
transparente com preenchimento branco para o tema escuro. Origem e autorização
estão em [assets/README.md](../assets/README.md) e `CLAUDE.md`.

O texto desenhado anteriormente foi substituído pela imagem, com 124 px de
largura e proporção preservada. Build Windows e clippy Linux passaram; a prévia
foi aberta e capturada em `package/preview-original-wordmark.png`.

## Protocolo

Levantado em 12/11/2026. Cada comando de escrita só entra no driver quando duas
fontes independentes concordam byte a byte: o código do CAM e a documentação
pública do liquidctl, usada somente como referência de fatos (sem copiar código).
As leituras também foram conferidas nos dispositivos do usuário.

### Método

1. `localizar_funcoes.py` leu o PE do `index.node` (134.494 funções no `.pdata`)
   e achou as funções que referenciam `kraken_x.rs`, `smartdevice.rs`, `v1.rs` e as
   mensagens de erro desses módulos, por instruções `lea [rip+disp]` e estruturas
   `Location` do Rust. Resultado: 8 funções, em 2 segundos, sem análise completa.
2. Desmontagem dessas funções com capstone (`ler_va.py`, `varrer_regiao.py`) e
   decompilação Ghidra sem análise automática (`DecompileList.java`).
3. Leitura somente de relatórios de entrada dos dois dispositivos, no Windows, com
   um leitor `hidapi` sem nenhuma chamada de escrita.

Os scripts e as saídas ficam em `package/ghidra-scripts/` e `package/analise/`.
Uma análise automática completa do binário foi tentada antes e abandonada: passou
de uma hora sem terminar e foi perdida quando a IDE fechou.

### Relatórios confirmados

| Comando | Bytes (relatório de 65 bytes, zeros no resto) | CAM (`index.node`) | Documentação | Hardware |
| --- | --- | --- | --- | --- |
| Velocidade fixa | `02 4d CANAL 00 DUTY` | `0x18142d748` (Kraken), `0x1813c2a51` (Smart Device) | igual | — |
| Ponto de curva do Kraken | `02 4d (0x80+BASE+i) TEMP DUTY`, um por ponto | laço em `0x18142db20`; soma `0x80`; recusa mais de 64 pontos; limita temperatura e duty a 0–100 (`f64` 100,0 em `0x181dc9478`); aplica piso | fan `0x80+i`, bomba `0xC0+i` | — |
| Base do canal | fan `0x00`, bomba `0x40` | o limite de 64 pontos é exatamente a distância `0x80`→`0xC0` | `base & 0x70` = `0x00`/`0x40` | — |
| Grade de temperaturas | 20–49 °C (1 em 1), 50–58 °C (2 em 2), 60 °C: 36 pontos | — | igual; mesma grade nos dois canais | — |
| Status do Kraken | `04`, líquido = b1 + b2 × 0,1, fan = b3‥b4, bomba = b5‥b6 (BE), firmware b11‥b14 | parser `0x1813b5960`, constante `f32` 0,1 em `0x182009970` | igual | `04 21 01 00 00 07 f2 … 06 00 00 02`: 33,1 °C, fan 0, bomba 2034 RPM, firmware 6.0.2 |
| Status do Smart Device | `04`, 21 bytes, um por fan: ruído b1, RPM b3‥b4, V = b7 + b8/100, A = b9 + b10/100, fan = b15 >> 4, modo = b15 & 3 | parser testa b15 | igual | fan 2: 1306 RPM, 9,85 V, DC; fan 3: 1341 RPM, DC; fan 1: sem fan |
| Inicialização do Smart Device | `01 5c`, depois `01 5d` | `0x1813c2933`, `0x1813c2961` | igual | — |
| Iluminação do Kraken | `02 4c CANAL|dir|mov MODO VEL` + logo GRB + 8 LEDs | `0x1813b684e` (`0x4c02`) | layout, canais logo `0x01`/anel `0x02` e ids de modo | — |
| Iluminação do Smart Device | `02 4b MODO VARIANTE VEL` + 57 bytes, depois `03` + 63 bytes | `0x1813b6258`, `0x1813c28e7` (`0x4b02`) | layout e ids de modo | — |

Observações do hardware em 12/11/2026: o conector de fan do Kraken relata 0 RPM
(nenhum fan ligado nele, a confirmar no CAM); o Smart Device tem fans DC nos
canais 2 e 3, nada no canal 1, firmware 1.7 e duas fitas HUE+.

### Escolhas do porte

- Curvas usam a grade de 36 pontos do firmware, interpoladas da curva configurada,
  sempre com 100% a 60 °C.
- Curva interna só com firmware principal ≥ 3; abaixo disso, rotação fixa.
- Pisos: bomba 50% (perfil de reset da documentação), fans 25% (o CAM declara
  `minActivePercentage` 0,2 para fans; o porte usa um piso maior para fans DC).
- Iluminação: confirmados no CAM o comando e o canal; os ids de modo vêm da
  documentação e precisam do ensaio visual no Ubuntu. Iluminação não afeta a
  refrigeração.

### Lacunas

- Qual camada do CAM escolhe o piso da bomba: o valor chega à função de curva
  por um enum de requisição montado fora do módulo `kraken_x.rs`.
- Comportamento do CAM ao fechar: o que fica gravado no dispositivo.
