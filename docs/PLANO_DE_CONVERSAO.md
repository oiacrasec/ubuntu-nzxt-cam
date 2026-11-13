# Execução da conversão e ponto de retomada

O [roadmap](ROADMAP.md) continua sendo a matriz de estado. Esta página registra
onde o trabalho parou e como continuar sem refazer etapas.

## Ponto de retomada — 12/11/2026

Último commit de código: `a447b11`. Árvore limpa. Nada foi executado no Ubuntu.
Verificado no Windows: `cargo test` (61, e 37 com `--no-default-features`) e
`cargo clippy --target x86_64-unknown-linux-gnu --all-targets -- -D warnings` nos dois builds.

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

## Próximos passos, na ordem

1. **Observar no CAM, com autorização do usuário para mudar a iluminação
   temporariamente:** a edição com "Personalizar cores LED" ligado (o clique de
   12/11 rolou a página e não ligou o interruptor; ele fica à direita da linha
   "#51007A" do detalhe do Anel) e o detalhe do modo "Temperatura do CPU" do
   Logótipo (escala de cores). Restaurar depois: Logótipo e Anel em Fixo `#51007A`,
   Fita em Respirar.
2. Ajustar o detalhe de iluminação ao observado. Já visto no CAM e ainda não
   reproduzido: na Fita, velocidade é um slider único com rótulo ("Velocidade Normal")
   abaixo da prévia, e "Add Color" é um botão ao lado do chip de cor.
3. Modos por temperatura (CPU/GPU) e Active Profile/Profile Sync.
4. Modos de áudio (Nivelado, Sincronização, Graves) com captura do PipeWire.
5. LEDs de memória G.Skill, placa-mãe ASUS Aura e placa de som AE-5 Plus, somente
   iluminação, abaixo do Smart Device (pedido do usuário). Memória via SMBus exige
   proteções contra escrita no SPD.
6. No Ubuntu, pelo usuário: seguir [Validação local](VALIDACAO_LOCAL.md).

## Regras que valem para a continuação

- Escrita em dispositivo de refrigeração só com bytes confirmados em duas fontes
  independentes; leituras conferidas no hardware sem escrita.
- Não copiar arte, logotipos ou fontes da NZXT; reproduzir layout, cores e rótulos.
- Fora da interface, a pedido do usuário: Áudio, Suporte e Beta.
- Commits em Conventional Commits, em inglês, com a linha `Co-Authored-By`.
