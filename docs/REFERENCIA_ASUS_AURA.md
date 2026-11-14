# Referência: iluminação ASUS Aura da placa-mãe e das memórias

Esta página reúne o que se sabe da iluminação da placa-mãe ROG MAXIMUS XI HERO e
das memórias G.Skill, controladas no Windows pelo Armoury Crate. Estado: **Fora do
escopo atual**. O plano de levá-las ao porte foi cancelado em 13/11/2026 pelo
risco ao hardware e pela dependência de um driver fora do kernel
([decisão](DECISOES.md#iluminação-asus-da-placa-mãe-e-das-memórias-cancelada--13112026)).
A página fica como registro para quem retomar o tema.

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

A identificação dos dois outros dispositivos não foi feita.

## SMBus: confirmação (13/11/2026)

Os LEDs da placa-mãe e das memórias usam o SMBus, confirmado por duas fontes: os
textos dos componentes instalados (lidos no binário, sem executá-lo) e a
documentação do OpenRGB. Nenhum acesso ao SMBus foi feito nesta análise.

| Item | Placa-mãe | Memórias |
| --- | --- | --- |
| Barramento no Windows | SMBus do Super I/O Nuvoton: `[CreateSmbus] Intel CreateSmbus using NCTSmbus` | SMBus do chipset Intel (`IntelSbSmbus`) |
| Controlador | ENE, firmware `AUMA0-E6K5` ou `AUMA0-E8K4` | ENE, `AUDA0-E6K5` |
| Endereço | descoberto pelo componente, lendo o registrador `0xC1` (`FindControllerAddress`, `Mainboard_Slave`); o OpenRGB cita `0x4E` como o mais comum | a lógica G.Skill varre o barramento (`ProbeSmbusAddress`) e muda o endereço de cada pente com duas escritas (`ChangeAddress`) |
| Driver no Linux | `i2c-nct6775`, que exige patch no kernel (não está no kernel oficial) | `i2c-i801`, no kernel oficial |

Os componentes usam a trava `Global\Access_SMBUS.HTP.Method` para não disputar o
barramento com outros programas no Windows.

## Caminho no Linux

| Dispositivo | Acesso | Situação |
| --- | --- | --- |
| Placa-mãe (LEDs próprios) | SMBus do Super I/O por `i2c-nct6775` e `i2c-dev` | Bloqueado: o driver exige patch no kernel, e o acesso exige root |
| Conectores ARGB da placa | hidraw do `0b05:18a3`, com regra udev, sem root | Viável quando houver algo ligado; protocolo da geração `18a3` não confirmado (o OpenRGB documenta a `18f3`, com mensagens HID de 65 bytes) |
| Memórias | SMBus do chipset por `i2c-i801` e `i2c-dev` | Alto risco: exige varrer o barramento e reendereçar os controladores no mesmo barramento do SPD; um erro pode corromper a configuração da memória |

## Se o tema voltar

| Caminho | Licença | Risco ao hardware |
| --- | --- | --- |
| SDK do OpenRGB (servidor local, porta `6742`) | continua MIT | nenhum código nosso no SMBus; o OpenRGB faz a varredura e o reendereçamento |
| Implementação própria a partir dos fatos | continua MIT | todo nosso; exige as regras abaixo |
| Copiar o código do OpenRGB | o projeto passa a GPL-2.0 | a operação arriscada roda no nosso binário; traduzir C++ para Rust pode introduzir erros |

Em qualquer caminho, a placa-mãe exige o `i2c-nct6775` com patch no kernel. Uma
hipótese não confirmada para as memórias: escrever no endereço de fábrica,
comum aos quatro pentes, mudaria os quatro juntos sem reendereçar; precisaria de
duas fontes antes de qualquer teste.

Regras para qualquer implementação futura:

1. Não varrer endereços do SMBus nem mudar endereços de controladores a partir do
   porte; usar só endereços confirmados por duas fontes e só os registradores de
   LED, com teste que recuse qualquer outro destino.
2. Nunca acessar endereços de SPD (`0x50`–`0x57`) nem `0x68`, endereço citado pelo
   OpenRGB em relatos de placas inutilizadas.
3. Exigir que o usuário instale o driver e autorize o acesso explicitamente, como
   a regra udev, mostrando o plano antes.
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
- [OpenRGB: visão geral do ASUS Aura (endereços)](https://openrgb-wiki.readthedocs.io/en/latest/asus/ASUS-Aura-Overview/)
- [OpenRGB: relato de memória Aura num Z390 com i9-9900K](https://gitlab.com/CalcProgrammer1/OpenRGB/-/issues/4058)

O código do OpenRGB (GPL) serve só como referência de fatos; não deve ser copiado.
