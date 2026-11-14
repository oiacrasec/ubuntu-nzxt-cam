# Referência: iluminação ASUS Aura da placa-mãe e das memórias

Esta página reúne o que se sabe da iluminação da placa-mãe ROG MAXIMUS XI HERO e
das memórias G.Skill, controladas no Windows pelo Armoury Crate, para levá-las ao
porte abaixo do Smart Device na página **Iluminação**. Estado: **Proposto**; nada
disto está implementado.

São produtos de outra marca, fora do escopo atual ([CLAUDE.md](../CLAUDE.md)); a
análise foi feita a pedido do usuário em 13/11/2026. Quando for implementada, a
tela segue o padrão visual das linhas do Kraken e do Smart Device, não o layout
da ASUS.

## Hardware de referência

| Item | Valor |
| --- | --- |
| Placa-mãe | ROG MAXIMUS XI HERO (Z390) |
| Controlador de LED da placa | Aura USB HID `0b05:18a3` |
| Memórias | Quatro G.Skill F4-3200C16-8GTZR (Trident Z RGB) |
| Barramento das memórias | SMBus Intel `8086:a323` do chipset |
| Software Windows | Armoury Crate 6.7.6.0, LightingService 3.11.08.0 |

## Efeitos da aba Efeitos Aura

Observados em 13/11/2026 com **ROG MAXIMUS XI HERO** e **Memória** sincronizados,
escolhendo cada efeito e abrindo cada lista; ao terminar, voltou a **Arco-íris**
com **Tempo** na posição do meio.

| Efeito | Opções |
| --- | --- |
| Estático | **Cor sólida** ou **Gradiente** (paleta de 2 cores e **Direção**, ativa só no gradiente) |
| Intermitente | as do Estático, **Aleatório** e **Tempo** |
| Estroboscópio | as do Intermitente |
| Ciclo de cor | nenhuma (**Sem defin. dispon.**) |
| Arco-íris | **Padrão** (lista de gradientes) e **Tempo** |
| Noite estrelada | **Fundo** (cor ou **Desligado**), **Efeito** (cor ou **Aleatório**), **Tempo** |
| Música | **Estilo** (Espectro de áudio, Beat Box, Cascata, Onda, Colisão), **Cor** (sólida, **Ciclo de cor** ou gradiente), **Sensibilidade** (50%), **Taxa de decaimento** (0%) |
| Smart | fonte **Por temp. da CPU** ou **Por utiliz. da CPU**, **Ponto inferior** 40 °C (verde) e **Ponto superior** 60 °C (vermelho), escala de 0 a 100 |
| Cor adaptável | área da tela capturada e refletida nos LEDs |
| Imersão | **Cor** (sólida, ciclo ou gradiente), **Sensibilidade**, **Taxa de decaimento** |
| Escuro (Desativado) | desliga a iluminação de todos os dispositivos sincronizados |
| Iluminação IA | gera efeitos a partir de um pedido, na nuvem (**Experimental**) |

**Tempo** tem cinco posições. A aba também oferece **Ef. avançados** (efeitos do
**Aura Creator**), **Efeitos ilum. jogo** (ligado) e **Repor predef.**

Equivalências com o porte: Smart corresponde aos modos por temperatura; Música e
Imersão, aos modos de áudio; Cor adaptável, ao modo Ambiente do CAM; Iluminação
IA não tem equivalente local.

## Como o Armoury Crate aplica os efeitos

O `LightingService` calcula o efeito e chama, por COM, um componente por tipo de
dispositivo. Os nomes abaixo vêm das bibliotecas de tipos (`.tlb`), lidas sem
executar os componentes.

| Componente | Interface principal | Extras |
| --- | --- | --- |
| `C:\Program Files\ASUS\AacMB\Aac3572MbHal_x64.exe` 1.6.1.9 (placa-mãe) | `IAacLedDevice`: `GetCapability`, `SetEffect(effectId, colors, numberOfColors)`, `Synchronize(effectId, milliseconds)`; `IAacLedDeviceOpt`: `SetEffectOptSpeed(effectId, colors, numberOfColors, speed, direction)` | efeito, velocidade e direção de **standby**; liga/desliga pela BIOS (`IAsusMotherboardHal`); número manual de LEDs (`IAacLedDeviceVariedLedCount`); temperatura em décimos de °C |
| `C:\Program Files\ASUS\ASUS_Aac_DRAM\Aac3572DramHal_x64.exe` 1.1.23.0 (memórias) | a mesma `IAacLedDevice`/`IAacLedDeviceOpt` | — |

Os efeitos chegam aos componentes como um identificador e uma lista de cores; os
identificadores e o que cada componente envia ao hardware não foram extraídos
(ambos são nativos).

## Captura USB (13/11/2026)

Com USBPcap 1.5.4, o tráfego do único controlador USB da máquina (Intel
`8086:a36d`, onde fica o `0b05:18a3`) foi gravado enquanto cada efeito básico era
escolhido por 6 s e o **Tempo** do Arco-íris passava pelas cinco posições; ao
final voltou a **Arco-íris** com **Tempo** no meio. Roteiro e análise em
`package/usb-captura/` (`varrer-aura.ps1`, `aura-USBPcap1.pcap`,
`aura-janelas.py`).

| Dispositivo no barramento | O que recebeu | Relação com o Aura |
| --- | --- | --- |
| `0b05:18a3` (Aura) | **nada** durante os 11 efeitos e as 5 velocidades | — |
| Kraken X e Smart Device | os comandos de refrigeração do CAM | nenhuma |
| Outro dispositivo, interrupção | `07 22 05` + cinco LEDs (índice e RGB), ~28 por segundo, em arco-íris contínuo e com o 3º LED sempre branco, antes e durante a varredura | nenhuma; parece outro periférico animado por software |
| Outro dispositivo, controle | três pedidos de classe periódicos e iguais | nenhuma |

Conclusão provável, a confirmar por uma segunda fonte: nesta placa os LEDs da
própria placa-mãe são comandados pelo SMBus, como as memórias, e o `0b05:18a3`
atende só os conectores ARGB endereçáveis, que estão vazios. A identificação dos
dois outros dispositivos não foi feita.

## Caminho no Linux

| Dispositivo | Acesso | Situação |
| --- | --- | --- |
| Placa-mãe (LEDs próprios) | SMBus pelo `i2c-i801` e `i2c-dev`, com root ou regra própria | Arriscado, como as memórias; a captura não mostrou tráfego USB |
| Conectores ARGB da placa | hidraw do `0b05:18a3`, com regra udev, sem root | Viável quando houver algo ligado; protocolo da geração `18a3` não confirmado (o OpenRGB documenta a `18f3`, com mensagens HID de 65 bytes) |
| Memórias | SMBus pelo `i2c-dev` | Arriscado. O chip RGB fica no mesmo barramento do SPD; um endereço errado pode corromper a configuração da memória |

Antes de implementar:

1. Confirmar por segunda fonte que os LEDs da MAXIMUS XI HERO usam o SMBus e em
   qual endereço; o SMBus não aparece em captura USB.
2. Não varrer endereços do SMBus: usar só endereços confirmados por duas fontes e
   só escrita nos registradores de LED, com teste que recuse qualquer outro destino.
3. Verificar se o `i2c-i801` acessa o SMBus no Ubuntu sem conflito com o ACPI.
4. Os efeitos animados (Música, Imersão, Noite estrelada, Smart) devem virar
   quadros enviados pelo controlador, como os modos de áudio do porte.

## Evidências locais

Em `package/`, fora do Git:

- `package/armoury/`: tela inicial (`ac-efeitos.png`), painel de cada efeito
  (`ac-<efeito>.png`, folha em `ac-paineis.png`) e listas abertas
  (`ac-lista-*.png`, folha em `ac-listas.png`).
- `package/win-observe.ps1`: para o Armoury Crate, use
  `-Process ApplicationFrameHost -Pasta armoury -Screen -Extra 0`; a janela é UWP
  e só aparece por cópia da tela.

## Fontes

- [OpenRGB: ASUS Aura USB](https://openrgb-wiki.readthedocs.io/en/latest/asus/ASUS-Aura-USB/)
- [OpenRGB: acesso ao SMBus](https://gitea.marinkevich.ru/grayhook/OpenRGB/raw/branch/master/Documentation/SMBusAccess.md)
- [OpenRGB: perguntas frequentes (i2c-dev, udev)](https://openrgb-wiki.readthedocs.io/en/latest/Frequently-Asked-Questions/)

O código do OpenRGB (GPL) serve só como referência de fatos; não deve ser copiado.
