# Referência: iluminação da Sound BlasterX AE-5 Plus

Esta página reúne o que se sabe da iluminação da placa de som AE-5 Plus para
levá-la ao porte, abaixo do Smart Device na página **Iluminação**. Estado:
**Proposto**. Nada disto está implementado, e o controle no Linux exige acesso
privilegiado ao hardware (ver [Caminho no Linux](#caminho-no-linux)).

A placa é de outra marca e fica fora do escopo atual do produto
([CLAUDE.md](../CLAUDE.md)); a análise foi feita a pedido do usuário em
13/11/2026 para uso futuro. Quando for implementada, a tela segue o padrão visual
das linhas do Kraken e do Smart Device no porte, não o layout da Creative.

## Hardware de referência

| Item | Valor |
| --- | --- |
| Placa | Sound BlasterX AE-5 Plus |
| Controlador PCI | `1102:0012`, subsistema `1102:0191` |
| Codec HDA | `1102:0011`, subsistema `1102:0191` |
| Software Windows | Sound Blaster Command 3.5.10.0 |
| Zona interna | LEDs da própria placa |
| Zona externa | Fita no conector da placa; o software mostra **LEDs** `10` |

O Sound Blaster Command identifica a AE-5 internamente por `0x1102`/`0x0051`
(`DEFINITION_PRODUCT_ID_SoundBlasterXAE5`), valor que não coincide com os IDs PCI.

## Opções da tela Iluminação

Observadas em 13/11/2026 abrindo cada lista e escolhendo cada efeito; ao terminar,
a placa voltou a **Personal**, **Ciclo**, **Slower**.

| Controle | Opções |
| --- | --- |
| Interruptor geral | liga ou desliga a iluminação |
| Perfil | Laranja solitário, Azul claro em ondas, Vermelho pulsante, Fogo em ondas, Verde em ondas, Azul em ondas, Amarelo astral, Personal; **+** cria outro |
| Zona | **Internal**, **External** (com interruptor próprio) |
| **LEDs** | número de LEDs da fita externa |
| **Motion** | Solo, Aurora, Atmosfera, Ciclo, Onda, Pulsar |
| **Speed** | Slower, Slow, Normal, Fast, Faster, Fastest |
| **Direction** | Right, Left, Saltar (vai e volta) |

Cada efeito mostra só os controles que usa:

| Motion | Speed | Direction | Cores |
| --- | --- | --- | --- |
| Solo | — | — | 1 (**Color Section (Solo)**) |
| Aurora | sim | — | automáticas |
| Atmosfera | — | — | 5 |
| Ciclo | sim | — | automáticas |
| Onda | sim | sim | 5 |
| Pulsar | sim | — | 5 |

O editor de cor tem **R**, **G**, **B**, hexadecimal, sete amostras, **+** para
guardar uma amostra, disco de cores e controle de intensidade do preto à cor.
Os perfis predefinidos parecem combinar um efeito com cores; a definição de cada
um não foi extraída.

## Modelo interno do Sound Blaster Command

A interface usa `Platform\Creative.Platform.Lighting.dll` (.NET). Os tipos e
constantes abaixo foram lidos por reflexão, sem executar o código.

| Tipo | Valores |
| --- | --- |
| `Pattern.Type` (interface) | Solo=0, Aurora=1, Wave=2, Pulsate=3, Mood=4 (**Atmosfera**), Cycle=5, PeakMeter=6 (não aparece na AE-5) |
| `CTLED_Pattern` (enviado) | None=0, ColourCycle=1, SpectrumAnalyzer=2, Static=3, Wave=4, Pulsate=5, Blink=6, Morph=7, Aurora=8, Cosmic=1000, WaveCycleBounce=1001, Impact=1002, PeakMeterSpectrum=2000 |
| `Direction.Type` | RightToLeft=0, LeftToRight=1, RightToLeftBounce=2, LeftToRightBounce=3 |
| `CTLED_ConnectionType` | BuiltIn=0, External=1 |
| `CTLED_HardwareType` | SeparateClockAndData=0, ClockEncodedWithData=1 |
| `CTLEDMGRCMD` | Test=0, GetLedSettings=1, SetLedSettings=2, RestartLedSettings=3, RestoreDefaultLedSettings=4, GetDeviceAttributes=1000 |

`CTLEDMGRCMDPARAM_SetLedSettings` carrega perfil, estado mestre (liga/desliga),
índice do LED, padrão, direção (com laço ou vai e volta), período da animação
em milissegundos ou ciclos por minuto, e cor (`CTColour`: R, G, B, A). A
velocidade da tela vira esse período; a tabela de conversão não foi extraída.

Caminho das chamadas: `LightingDevice.Set` → `LEDManager` → `CTLEDMgr.dll` (COM,
CLSID `3370DE2D-576E-421D-9D04-E01B84D0E3EA`) → `CTDrvLedMgr.dll` → driver da placa.
`ClplLEDController.dll`, `CTLEDMgr.dll` e `CTDrvLedMgr.dll` são nativos e não foram
analisados.

## Caminho no Linux

O merge request
[OpenRGB !3658](https://gitlab.com/CalcProgrammer1/OpenRGB/-/merge_requests/3658),
aberto e não aceito até 25/09/2026, descreve o controle verificado numa AE-5
física. São fatos de referência; o código do OpenRGB (GPL) não deve ser copiado.

| Zona | Hardware | Acesso no Linux |
| --- | --- | --- |
| Interna | 5 LEDs APA102 | SPI por bit-banging no registrador GPIO no deslocamento `0x320` da BAR2 PCIe, mapeada por `/dev/mem` ou pelo arquivo `resource2` do sysfs |
| Externa | Fita WS2812B, 1 a 100 LEDs | A ponte CA0113 serializa dados GRB a 800 kHz a partir de fluxos DMA HDA marcados em BAR2 `0x104`; o merge request usa os arquivos `ae5_strip_leds` e `ae5_strip_num_leds` do `snd-hda-codec-ca0132` |

Pontos em aberto antes de implementar:

- Não foi confirmado se `ae5_strip_leds` existe no kernel oficial ou só num patch.
- Mapear a BAR2 exige root e um bloco `unsafe` (mmap), contrário ao
  `unsafe_code = "forbid"` do projeto. Alternativas: um helper privilegiado
  separado, como o da regra udev, ou escrita por `pwrite` em `/dev/mem`, sujeita
  a `CONFIG_STRICT_DEVMEM`.
- Os efeitos (Aurora, Atmosfera, Ciclo, Onda, Pulsar) parecem ser animados por
  software no Windows, porque os LEDs internos são acionados por GPIO; no porte,
  seriam quadros enviados pelo controlador, como os modos de áudio.
- A escrita errada num registrador da placa pode afetar o áudio; qualquer
  implementação precisa de proteção equivalente à do SPD da memória.

## Evidências locais

Em `package/`, fora do Git:

- `package/sbcommand/`: capturas de cada lista e de cada efeito
  (`sb-motion-*.png`, `sb-speed-lista.png`, `sb-perfil-*.png`,
  `sb-zona-lista.png`, `sb-direcao-lista.png`).
- `package/win-observe.ps1`: captura qualquer janela por nome de processo
  (`-Process`), com cliques e rolagem; `-Screen` copia a tela em volta da janela
  para incluir listas suspensas.

Para listar de novo os tipos e constantes da DLL, carregue
`Creative.Platform.Lighting.dll` com `[Reflection.Assembly]::LoadFrom` no
PowerShell e percorra `GetTypes()`.
