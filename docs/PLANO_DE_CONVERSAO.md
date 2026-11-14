# Execução da conversão e ponto de retomada

O [roadmap](ROADMAP.md) continua sendo a matriz de estado. Esta página registra
onde o trabalho parou e como continuar sem refazer etapas.

## Ponto de retomada — 12/11/2026

Último commit de código anterior à continuação: `a447b11`; handoff em `4c9ece0`.
Na continuação entraram documentação, prévia Windows, interface, Profile Sync
(pilha de ativações, vários programas por regra, dias da semana e horários que
cruzam a meia-noite) e atualização pelo commit de produção; ver a tabela abaixo.
Nada foi executado no Ubuntu.
Verificado no Windows em 13/11/2026: `cargo test --no-default-features --features gui` (78),
`cargo test --no-default-features` (76) e
`cargo clippy --target x86_64-unknown-linux-gnu --all-targets -- -D warnings` nos dois builds.

Também foi acrescentada, a pedido do usuário, uma [prévia visual no Windows](USO.md#conferir-o-layout-no-windows):
`cargo build --no-default-features --features gui`, depois `preview-windows.cmd`.
Usa a mesma GUI com dados simulados, estado só em memória e transporte que recusa
E/S; não usa o controlador real. A janela foi aberta e capturada em
`package/preview-windows.png`; a revisão mais recente usa `package/preview-final-*.png`.
Os testes incluem isolamento da prévia, preflight do Profile Sync, recusa de saída
térmica com falha, codificação por LED da Fita em Respirar e atualização via Git local.
Isso não valida o funcionamento no Ubuntu.

Ícones originais de bandeja e aplicativo foram incluídos por pedido expresso do
usuário, com origem em [assets/README.md](../assets/README.md) e autorização em
`CLAUDE.md`. A prévia foi reconstruída e aberta com o ícone na janela, registrada
em `package/preview-original-icon.png`. O instalador publica o PNG no hicolor;
a bandeja usa pixmap embutido. A integração real com o desktop Ubuntu está pendente.

Na revisão lado a lado pedida pelo usuário, foram corrigidos ícones de rede e
ordenação, seis colunas de processos, cliques/detalhes de todos os tipos de
especificações e expansão independente dos canais de iluminação. Mini Mode foi
retirado da interface. Profile Sync fica antes de Sobre; Sobre segue o updater
da Facecam (revisão `d32e400`; a cópia local em `package/` foi apagada em 13/11/2026). A consulta usa a branch
padrão main/master, não releases. Instalação, rollback e reinício reais no Ubuntu
ainda não foram exercitados. O CAM original não foi modificado nessa revisão.

| Commit | Entrega |
| --- | --- |
| `e096b08` | Documentação base, escopo e referência Windows |
| `469fed7` | Controlador, IPC, CLI, configuração, sensores, governador, udev, GUI inicial |
| `827cb53` | Scripts de instalação/remoção/testes, CI, `USO.md`, roteiro de validação |
| `d0ab508`, `46671ed` | Catálogo EN-US, arquitetura e decisões |
| `3a32c1f` | Drivers Kraken X e Smart Device V1 com protocolo confirmado em 3 fontes |
| `9a53aa3` | Interface no layout do CAM, especificações, discos, processos, firmware, Mini Mode |
| `4ef942d` | `.gitattributes` com LF |
| `a447b11` | Menus de iluminação do CAM, detalhe do canal, cor por LED |

## Ferramentas de análise (fora do Git, em `package/`)

- `package/NZXT CAM 4.76.8/`: cópia da instalação (referência).
- `package/ghidra-scripts/`: `localizar_funcoes.py` (acha funções por `.pdata` + `lea`
  em segundos), `ler_va.py` (dados por VA e desmontagem capstone), `varrer_regiao.py`,
  `chamadores.py`, `DecompileList.java` e `rodar_alvos.cmd` (Ghidra sem análise
  automática, retomável, um arquivo por função). Não rodar análise completa do
  `index.node`: são 134.494 funções e passa de uma hora.
- `package/ghidra-alvos/`: projeto Ghidra importado sem análise, reaproveitável.
- Ghidra 12.1.4 em `D:\Projetos\PycharmProjects\ghidra_12.1.4_PUBLIC`; REA compilada em
  `D:\Projetos\PycharmProjects\rea`; Rust 1.100 no perfil do usuário.
- Capturas do CAM: somente a janela, por `PrintWindow`, com cliques via `SetCursorPos`
  e `mouse_event` nas coordenadas da janela; devolver o cursor ao terminar.

## Continuação de 13/11/2026 (sem o usuário, só Windows)

Commits enviados ao GitHub pelo usuário em 13/11/2026:

| Commit | Entrega |
| --- | --- |
| `418b842` | Trabalho do Codex concluído: Profile Sync, atualização, prévia Windows |
| `d9e1f53` | Diálogos do Profile Sync alinhados e traduzidos |
| `cb72849` | Diálogo **Com programa** em duas colunas (conferido na prévia) |
| `bd1f236` | Gráfico de Resfriando no estilo do CAM e seletor de fonte (conferido na prévia) |
| `571dd4b` | Modos de iluminação por temperatura (CPU/GPU) com editor de limites (conferido na prévia) |
| `ac2d056` | Teste do reenvio por temperatura; prévia Windows sem avisos; roteiro 15–17 |

Verificado no Windows: `cargo test --no-default-features --features gui` (77),
`cargo test --no-default-features` (76) e clippy para Linux com `-D warnings`
nos dois builds. A conferência visual usou a prévia com dados simulados,
capturando só a janela dela. Nada foi executado no Ubuntu.

## Próximos passos, na ordem

0. **Concluído em 13/11/2026:** gráfico de Resfriando no estilo do CAM (captura do
   usuário): fundo `(18,18,20)` sem moldura; só linhas horizontais suaves em 0, 25,
   50, 75 e 100%; eixo de 10° a 100° de 10 em 10; linha e pontos verdes
   `(33,211,115)`; linha vertical tracejada na temperatura atual com rótulo
   "Líquido: 35°" acima; seletor "Fonte de temperatura" centralizado acima do
   gráfico, dentro do detalhe do canal. Função: `curve_editor` em `src/gui.rs`;
   `cooling_detail` recebe a fonte. Manter pisos e 100% na temperatura crítica,
   mesmo sem as linhas laranja de hoje.

1. **Concluído na continuação:** observar "Personalizar cores LED" ligado no
   Anel e "Temperatura do CPU" no Logótipo, com autorização explícita do usuário.
   Restauração confirmada na interface: Logótipo e Anel em Fixo `#51007A`,
   personalização do Anel desligada e Fita em Respirar. Evidências e limites em
   [Referência Windows](REFERENCIA_WINDOWS.md#detalhe-da-iluminação-observado-no-cam).
2. **Implementado, sem validação no Ubuntu:** slider único de velocidade com
   rótulo abaixo da prévia, amostras de cor e botão de adicionar ao lado, e clique
   no segmento da prévia para editar um LED. Os seletores e desenhos são próprios.
3. **Concluído em 13/11/2026:** modos por temperatura (CPU/GPU), com editor de limites. Active Profile/Profile Sync já está implementado;
   validar regras por programa/horário e retorno à seleção anterior no Ubuntu.
4. **Concluído em 13/11/2026, sem validação no Ubuntu:** menus e opções da
   Iluminação iguais aos do CAM (conferidos modo a modo no CAM real), modos de
   áudio (Nivelado, Sincronização, Graves, Ganho) com captura por `parec` ou
   `pw-record` em `src/audio.rs`, Onda da Fita e Respirar por LED no Anel.
   Detalhes em [Referência Windows](REFERENCIA_WINDOWS.md#menus-e-opções-de-cada-modo-13112026).
5. **Revisar no Ubuntu, com o hardware:** o CAM 4.76.9 reenvia ao Kraken, a cada
   segundo, curvas de 17 pontos (20–100 °C) e rotação fixa de fan e bomba; o porte
   grava a curva de 36 pontos (20–60 °C) só quando muda. Decidir com base nas
   regras de segurança térmica antes de mudar
   ([Referência Windows](REFERENCIA_WINDOWS.md#outros-programas-e-refrigeração-observados-na-captura)).
6. LEDs da placa de som AE-5 Plus, somente iluminação, abaixo do Smart Device
   (pedido do usuário; [referência](REFERENCIA_SOUND_BLASTER.md)). Placa-mãe e
   memórias foram canceladas em 13/11/2026
   ([decisão](DECISOES.md#iluminação-asus-da-placa-mãe-e-das-memórias-cancelada--13112026)).
7. No Ubuntu, pelo usuário: seguir [Validação local](VALIDACAO_LOCAL.md), incluindo
   instalação registrada, consulta de produção, publicação/rollback e reinício
   seguro a partir de Sobre. Campos indisponíveis nas especificações/processos
   são exibidos como `—`, sem simular leituras reais.

## Regras que valem para a continuação

- Escrita em dispositivo de refrigeração só com bytes confirmados em duas fontes
  independentes; leituras conferidas no hardware sem escrita.
- Não copiar arte, logotipos ou fontes da NZXT; reproduzir layout, cores e rótulos.
- Fora da interface, a pedido do usuário: Áudio, Suporte e Beta.
- Commits em Conventional Commits, em inglês, com a linha `Co-Authored-By`.
