# Validação local

Esta página guarda o roteiro de validação no Ubuntu e os resultados de cada
execução. Uma etapa só é marcada como validada com data, revisão, hardware e
resultado registrados abaixo.

## Ambiente de referência

| Item | Valor |
| --- | --- |
| CPU | Intel Core i9-9900K (`coretemp`) |
| GPU | NVIDIA GeForce RTX 4070 SUPER (NVML do driver proprietário) |
| Placa-mãe | ASUS ROG Maximus XI Hero |
| Dispositivos | Kraken X (`1e71:170e`, serial `6D8B4469545`), Smart Device V1 (`1e71:1714`, serial `876F27655052`) |
| Sistema | Ubuntu 24.04+ x86-64 |

Os seriais foram lidos no Windows em 12/11/2026; confira em `--list-devices`.

## Roteiro no Ubuntu

Siga em ordem. As etapas 1 a 4 não escrevem nos dispositivos. Antes da etapa 5,
feche liquidctl, CoolerControl ou qualquer outro programa que use os dispositivos.

1. **Build e testes.** `./test-desktop-app.sh` termina sem erro.
2. **Descoberta.** `cargo run --locked -- --list-devices` lista `kraken_x:…` e
   `smart_device:…` com o `/dev/hidrawN` correspondente.
3. **Sensores.** `cargo run --locked -- --sensors` mostra temperatura de pacote e
   de núcleos da CPU, a RTX 4070 SUPER com temperatura e carga, RAM e rede.
   Compare com `sensors` (lm-sensors) e `nvidia-smi`.
4. **Permissões.** Instale (`./install-desktop-app.sh`), abra o aplicativo e prepare
   as permissões em **Configurações**. Confirme `ls -l /dev/hidraw*` com ACL do
   usuário (`getfacl /dev/hidrawN`) e o estado **Pronto** nos dois dispositivos.
5. **Leitura dos dispositivos.** Em **Monitoramento**, compare com a leitura do
   Windows de 12/11/2026: líquido ~33 °C em repouso, bomba ~2030 RPM, fan do Kraken
   0 RPM (nenhum fan no conector do Kraken); Smart Device com fan 2 ~1300 RPM,
   fan 3 ~1340 RPM e fan 1 vazio. Firmware do Kraken 6.0.2.
6. **Curva no Kraken (fonte Líquido).** Aplique **Performance** na bomba. O RPM sobe
   em poucos segundos. Encerre com `kill -9` o processo e confirme que a bomba
   continua na mesma rotação.
7. **Governador (fonte CPU).** Aplique **Fixed 40%** com fonte CPU no fan do Kraken;
   depois **Silent** com fonte CPU e rode uma carga (`stress-ng --cpu 0 -t 60`).
   O fan acompanha a temperatura e desce gradualmente ao terminar.
8. **Falha de fonte.** Com fonte GPU, descarregue a NVML (ou use uma GPU inexistente
   pela configuração) e confirme **Segurança: fonte de temperatura indisponível**
   com 100%.
9. **Smart Device.** Aplique **Fixed 30%** em um fan e confirme o RPM. Saia por
   **Sair** na bandeja e confirme os fans em 100%.
10. **Iluminação.** Teste **Fixed**, **Breathing** e **Spectrum Wave** em cada canal e
    compare com o CAM no Windows.
11. **Suspensão.** Suspenda e retome; os dispositivos voltam a **Pronto** e o perfil
    é reaplicado.
12. **Interface.** Modo Mini, °C/°F, idioma EN-US, início com a sessão, fechar para
    a bandeja.

Registre também o que falhar: comando, mensagem exibida, `--status` e, para
falhas de protocolo, a saída de `journalctl --user -b | grep nzxt-cam`.

## Resultados

### Leitura no Windows — 12/11/2026

Leitor `hidapi` somente leitura, com o CAM 4.76.8 aberto, sem escrita nos
dispositivos. Relatórios brutos e decodificação em
[Protocolo](REFERENCIA_WINDOWS.md#protocolo); eles também são os vetores dos testes
de `protocol.rs`, `kraken_x.rs` e `smart_device.rs`.

### Ubuntu

Nenhuma execução no Ubuntu foi registrada até 12/11/2026. Os testes automáticos
rodaram no Windows (Rust 1.100) com `cargo test`, com e sem recursos padrão, e
`cargo clippy` para `x86_64-unknown-linux-gnu`; isso não valida hidraw, hwmon,
udev, NVML, bandeja nem a janela no Linux.
