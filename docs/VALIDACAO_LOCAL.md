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
12. **Interface.** °C/°F, idioma EN-US, início com a sessão, fechar para a bandeja,
    setas e ordenação dos processos, todos os detalhes de especificações e vários
    canais de iluminação expandidos ao mesmo tempo. Mini Mode foi removido.
13. **Profile Sync.** Salve perfis de iluminação/refrigeração, combine-os em um
    perfil mestre e confira a ativação manual, por programa e por horário, incluindo
    meia-noite e retorno a Default. Um perfil inválido ou canal ausente deve recusar
    a aplicação e não ser exibido como ativo. Faça isso somente no ensaio de hardware
    autorizado, observando as regras de segurança térmica das etapas anteriores.
14. **Atualização do aplicativo.** Após instalar pelo script, confira o registro e
    `--build-info`. Em Sobre, consulte a branch padrão do projeto. Só instale uma
    revisão revisada, com backup da configuração; confira log, checksum e recursos.
    Verifique a restauração do executável/registro em falha de publicação em uma
    instalação de teste isolada. O controlador deve continuar ativo durante o build;
    o reinício deve esperar o estado térmico de saída e liberar o controlador anterior.

15. **Gráfico de Resfriando.** Abra o detalhe de um canal: fonte de temperatura
    acima do gráfico, linhas em 0/25/50/75/100%, eixo de 10° a 100°, curva verde
    até 100° e marcador tracejado com a leitura atual. Arraste um ponto de
    **Personalizado** e confira que não desce abaixo do piso do canal.
16. **Iluminação por temperatura.** No Logótipo, escolha **Temperatura do CPU**:
    a lista "é superior a" substitui a prévia. Rode uma carga
    (`stress-ng --cpu 0 -t 60`) e confira a troca de cor ao cruzar 65 °C e a
    volta ao terminar. Confira também **Temperatura da GPU** e compare a cor
    com o CAM no Windows (faixas e igualdade ainda não confirmadas no CAM).
    O controle de brilho deve ficar apagado nesse modo, como no CAM.
18. **Modos de áudio e Onda.** Instale `pulseaudio-utils`, toque uma música e
    escolha **Nivelado**, **Sincronização** e **Graves** no Anel e **Ganho** na
    Fita: os LEDs devem acompanhar o volume (e os graves, em Graves) e apagar no
    silêncio. Sem o `parec`, o aplicativo mostra o erro com o pacote a instalar.
    Na Fita, **Onda** deve animar as cores escolhidas LED a LED. Confira também,
    no Anel, **Alternado** com **Movimento** e **Respirar** com
    **Personalizar cores LED**.
17. **Diálogos do Profile Sync.** Em um perfil diferente de Default, abra
    **Com programa** (busca, **Procurar programas**, conectar/remover) e
    **Em horário específico** (dias em quatro colunas, Início/Fim alinhados,
    **Dia inteiro**, **+1 dia** quando o fim é menor que o início).

Registre também o que falhar: comando, mensagem exibida, `--status` e, para
falhas de protocolo, a saída de `journalctl --user -b | grep nzxt-cam`.

## Resultados

### Leitura no Windows — 12/11/2026

Leitor `hidapi` somente leitura, com o CAM 4.76.8 aberto, sem escrita nos
dispositivos. Relatórios brutos e decodificação em
[Protocolo](REFERENCIA_WINDOWS.md#protocolo); eles também são os vetores dos testes
de `protocol.rs`, `kraken_x.rs` e `smart_device.rs`.

### Ubuntu — 13/11/2026

Primeira execução no Ubuntu, revisão `81e5b3f` e correção da fonte padrão do
Smart Device. Rust 1.99.1, kernel 7.0.0-43.

| Etapa | Resultado |
| --- | --- |
| 1. Build e testes | `cargo test`: 88 testes; `--no-default-features`: 87; `cargo clippy --all-targets` sem avisos |
| 2. Descoberta | `kraken_x:6D8B4469545` em `/dev/hidraw10`, `smart_device:876F27655052` em `/dev/hidraw9` |
| 3. Sensores | CPU pacote 43 °C; RTX 4070 SUPER 46 °C, igual ao `nvidia-smi`; RAM, rede, NVMe e discos sem erro. `lm-sensors` não instalado, CPU não comparada |
| 4. Permissões | Antes da regra, os dois em `permission_denied` com a mensagem de Configurações. Regra instalada pela interface; ACL `user:cesar:rw-` nos dois nós e estado **Pronto** |
| 5. Leitura | Kraken: líquido 35,1 °C, bomba 1868 RPM, fan 0 RPM, firmware 6.2 (exibição do CAM para `06 00 00 02`). Smart Device: fan 1 vazio, fans 2 e 3 em ~1500 RPM a 100%, firmware 1.7 |

Falha encontrada na etapa 5 e corrigida: sem perfil salvo, os fans do Smart Device
herdavam a fonte **Líquido**, que o dispositivo não mede, e ficavam em
**Segurança: fonte de temperatura indisponível** a 100%. Agora o padrão desse canal
é **Silent** pela CPU (`CoolingChannel::default_cooling`). Depois da correção, com
a CPU a 41 °C, os três fans foram para 25% (fans 2 e 3 em ~1000 RPM), sem segurança,
e o Kraken manteve a curva interna pelo líquido.

Etapas 6 a 18 ainda não foram executadas no Ubuntu.

### Especificações do sistema — 13/11/2026

Os 14 itens da lista abriram o detalhe correto, mas faltavam dados disponíveis sem
root. Depois da correção, os 17 itens (quatro módulos de RAM em vez do total)
foram clicados na janela e mostraram:

| Item | Resultado |
| --- | --- |
| CPU | Intel, nome do código Coffee Lake S, 3600 MHz padrão, clock atual. Soquete, energia e TDP em `—` (exigem root) |
| GPU | RTX 4070 SUPER, clock atual e energia em W pela NVML |
| Placa-mãe | ROG MAXIMUS XI HERO, chipset Intel Coffee Lake S, southbridge Intel Z390, BIOS 0805 de 01/29/2019 |
| RAM | Quatro G-Skill F4-3200C16-8GTZR, 8 GB, 2133 MT/s, DDR4. Tempos em `—` |
| Armazenamento | Nomes completos (antes truncados em 16 caracteres nos discos ATA), fabricantes Western Digital, ADATA, Seagate e Samsung (antes `—` ou `ATA`) e formatos de todas as partições |
| Áudio | Formato e canais das quatro placas pelo `pactl` |
| Rede | Wi-Fi com o IPv4 da interface; Ethernet desconectada em `—` |

Iluminação do Kraken X: anel e logo não mudam com nenhum comando (Fixo em
vermelho, verde e azul, brilho 20% e Onda de espectro), embora o dispositivo aceite
as escritas e a refrigeração funcione. No Windows o sintoma era o mesmo com o CAM.
Teste pendente: desligar a fonte por 30 s e repetir. A Fita do Smart Device responde.
