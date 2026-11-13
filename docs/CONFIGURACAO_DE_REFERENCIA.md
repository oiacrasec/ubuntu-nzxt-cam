# Configuração de referência

Para repetir no NZXT CAM Windows a refrigeração usada no Ubuntu, configure a bomba
do Kraken X e os fans do Smart Device V1 em **Personalizado**, com os pontos abaixo.
Esta página registra a configuração do hardware de referência e o que muda ao
levá-la para o Windows.

## Origem e resultado

| Item | Valor |
| --- | --- |
| Data | 13/11/2026 |
| Revisão | `e3fae6c`, instalada pela atualização em **Sobre** |
| Ambiente | [Ambiente de referência](VALIDACAO_LOCAL.md#ambiente-de-referência): i9-9900K, RTX 4070 SUPER, Kraken X (firmware 6.2), Smart Device V1 (firmware 1.7) |
| Configuração | Perfil **Default**, sem perfil salvo por canal: vale o padrão de cada canal ([Uso](USO.md#segurança-térmica)) |
| Leitura em repouso | Líquido 36,7 °C; bomba 1873 RPM; CPU (pacote) 40 °C; fans 2 e 3 em 25% (845 e 875 RPM) |

O usuário relatou temperatura estável com essa configuração. A estabilidade não
foi medida ao longo do tempo nem sob carga.

## Canais

| Dispositivo | Canal | Conectado |
| --- | --- | --- |
| Kraken X | Bomba | Bomba do water cooler |
| Kraken X | Fan | Nada (0 RPM) |
| Smart Device V1 | Fan 1 | Nada |
| Smart Device V1 | Fan 2 e Fan 3 | Fans do gabinete |
| Smart Device V1 | LED | Fitas HUE+ |

## Bomba do Kraken X

Fonte **Líquido**, curva **Silencioso** com o piso da bomba (50%), executada pelo
próprio firmware do Kraken mesmo com o aplicativo fechado.

| Líquido | 35 °C ou menos | 40 °C | 45 °C | 50 °C | 55 °C | 60 °C ou mais |
| --- | --- | --- | --- | --- | --- | --- |
| Duty | 50% | 52% | 53% | 55% | 75% | 100% |

Pontos da curva: (35 °C, 50%), (50 °C, 55%), (55 °C, 75%), (60 °C, 100%).
O fan do Kraken, se for conectado, segue a mesma forma com piso de 25%:
(35 °C, 25%), (50 °C, 55%), (55 °C, 75%), (60 °C, 100%).

## Fans do Smart Device V1

Fonte **CPU** (temperatura do pacote), curva **Silencioso** esticada até a
temperatura crítica da CPU, 90 °C (`preset` em `src/curves.rs`).

| CPU | 52,5 °C ou menos | 55 °C | 60 °C | 65 °C | 70 °C | 75 °C | 80 °C | 85 °C | 90 °C ou mais |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Duty | 25% | 28% | 35% | 42% | 48% | 55% | 68% | 83% | 100% |

Pontos da curva: (52,5 °C, 25%), (75 °C, 55%), (82,5 °C, 75%), (90 °C, 100%).
O Fan 1 está vazio; a configuração dele não altera nada.

## Iluminação

| Canal | Modo | Cor | Brilho |
| --- | --- | --- | --- |
| Kraken X: **Logótipo** | **Fixo** | `#51007A` | 100% |
| Kraken X: **Anel** | **Fixo** | `#51007A` | 100% |
| Smart Device V1: **Fita** | **Respirar**, velocidade **Normal** | `#51007A` | 20% |

Limitação conhecida neste hardware: o Anel e o Logótipo do Kraken não mudam com
nenhum comando de iluminação, no Ubuntu nem no Windows
([Validação local](VALIDACAO_LOCAL.md#especificações-do-sistema--13112026)).

## Diferenças no NZXT CAM Windows

- **Use Personalizado.** No porte, Silencioso com fonte CPU estica a curva do
  líquido até 90 °C. A curva que o CAM Windows usa para CPU não foi verificada;
  os pontos acima reproduzem a do porte em qualquer caso.
- **Descida gradual**: o governador do porte reduz o duty em no máximo 5 pontos
  por segundo (`fall_step`) e sobe sem atraso, com ciclo de 1 s. Não foi
  verificado se o CAM Windows suaviza a descida.
- **Bomba**: no Windows, com o CAM 4.76.8, a bomba girava ~2030 RPM com o líquido
  a 33 °C ([leitura de 12/11/2026](VALIDACAO_LOCAL.md)); no porte, ~1870 RPM com o
  líquido a 36–37 °C. O CAM 4.76.9 envia ao Kraken uma rajada de 36 relatórios por
  segundo ([Referência Windows](REFERENCIA_WINDOWS.md)); a relação dessa rajada com
  a rotação mais alta não foi verificada.
- **Saída do aplicativo**: no porte, os fans do Smart Device vão a 100% ao sair
  normalmente, porque o dispositivo não tem curva interna. O comportamento do CAM
  Windows ao fechar não foi verificado.

A aplicação desta configuração no CAM Windows ainda não foi feita. Ao fazer,
registre data, versão do CAM e leituras de repouso nesta página.
