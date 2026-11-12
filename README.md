# NZXT CAM para Ubuntu

Controlador nativo em Rust que porta o NZXT CAM para Ubuntu 24.04+: monitoramento
do PC, curvas de bomba e fans, iluminação RGB e perfis, com o layout do CAM.
Dispositivos-alvo: NZXT Kraken X (X52/X62/X72) e NZXT Smart Device V1.

**Estado: em planejamento.** Ainda não há código executável. A arquitetura e a
referência Windows estão documentadas; a implementação começa pela fundação do
controlador. Veja o [roadmap](docs/ROADMAP.md).

## Escopo

| Área do CAM | No porte |
| --- | --- |
| Monitoramento de CPU, GPU, RAM, armazenamento e rede | Proposto |
| Refrigeração: perfis Silent, Performance, Fixed e Custom por canal | Proposto |
| Iluminação: modos do Kraken X (Ring, Logo) e do Smart Device | Proposto |
| Ruído do gabinete (sensor do Smart Device V1) | Proposto |
| Profile Sync, Modo Mini, bandeja e início com a sessão | Proposto |
| Atualização de firmware, conta, telemetria e loja | Fora do escopo |
| Kraken com LCD, periféricos, monitores, PSU, áudio e captura | Fora do escopo atual (sem hardware para validar) |

## Requisitos previstos

Ubuntu 24.04+ x86-64 e Rust 1.95 ou posterior. O acesso aos dispositivos usa
hidraw; uma regra udev instalada com confirmação libera o acesso ao usuário da
sessão, sem rodar o aplicativo como root. Sensores vêm do hwmon do kernel e,
para GPUs NVIDIA, da NVML do driver instalado.

## Documentação

- [Manual completo](docs/README.md)
- [Arquitetura e segurança térmica](docs/ARQUITETURA.md)
- [Referência Windows: NZXT CAM 4.76.8](docs/REFERENCIA_WINDOWS.md)
- [Roadmap e critérios de prontidão](docs/ROADMAP.md)
- [Decisões de implementação](docs/DECISOES.md)

Projeto independente, sem afiliação com a NZXT. Código sob [MIT](LICENSE).
A cópia do CAM e as evidências locais ficam em `package/`, ignorado pelo Git, e não
integram a distribuição.
