# Roadmap, paridade e prontidão

O alvo é paridade funcional e visual com NZXT CAM 4.76.8.70 para o Kraken X
(X52/X62/X72) e o Smart Device V1, em Rust para Ubuntu 24.04+. Esta página
distingue código existente, evidência, lacunas e condições de aceitação.

## Matriz atual — 12/11/2026

| Dimensão | Estado | Próxima verificação |
| --- | --- | --- |
| Referência Windows | Protocolo levantado | CAM, documentação e leitura real conferidos; [Protocolo](REFERENCIA_WINDOWS.md#protocolo) |
| Controlador, IPC, CLI, configuração | Implementado, sem validação no Ubuntu | Testes unitários no Windows; build e execução no Ubuntu pendentes |
| Permissões (regra udev) | Implementado, sem validação no Ubuntu | Acesso ao hidraw como usuário comum |
| Sensores (CPU, GPU, RAM, disco, rede) | Implementado, sem validação no Ubuntu | Leitura no i9-9900K e na RTX 4070 SUPER |
| Governador e curvas | Implementado, sem validação no Ubuntu | Lógica testada sem hardware; resposta real pendente |
| Kraken X: status | Implementado, formato confirmado no hardware (Windows) | Leitura no Ubuntu pelo hidraw |
| Kraken X: curva de segurança e perfis | Implementado, bytes confirmados em CAM e documentação | Perfil interno sobrevive ao encerramento do app |
| Kraken X: iluminação Ring/Logo | Implementado; ids de modo só pela documentação | Comparação visual impossível no hardware de referência: LEDs do Kraken com defeito ([validação](VALIDACAO_LOCAL.md#iluminação-do-kraken-x--13112026)) |
| Smart Device V1: fans e ruído | Implementado, formato confirmado no hardware (Windows) | Rotação fixa e leitura no Ubuntu |
| Smart Device V1: iluminação | Implementado; ids de modo só pela documentação | Comparação visual com o CAM |
| Iluminação: listas de modos, detalhe, cor por LED | Implementado, sem validação no Ubuntu; edição por LED e slider observados no CAM | Conferir interação e aparência no Ubuntu; [observações da referência](REFERENCIA_WINDOWS.md#detalhe-da-iluminação-observado-no-cam) |
| Iluminação: modos por temperatura (CPU/GPU) | Implementado, sem validação no Ubuntu | Limites e cores padrão observados no CAM; troca por faixa e igualdade são suposições a conferir; GPU não observado no CAM |
| Iluminação: modos de áudio (Nivelado, Sincronização, Graves, Ganho) e Onda da Fita | Implementado em 13/11/2026, sem validação no Ubuntu | Menus e opções conferidos no CAM; Nivelado, Sincronização e Graves seguem a captura USB do CAM 4.76.9; captura de som por `parec`/`pw-record`, Ganho e Onda a ensaiar no Ubuntu |
| Active Profile e Profile Sync | Implementado, sem validação no Ubuntu | Perfis nomeados, ativação manual, por programa e horário; regras e preflight testados sem hardware |
| LEDs da placa de som AE-5 Plus | Proposto, pedido do usuário em 12/11/2026 | Só iluminação, abaixo do Smart Device. Efeitos e caminho no Linux levantados em [referência](REFERENCIA_SOUND_BLASTER.md); exige acesso privilegiado à BAR2 |
| LEDs da placa-mãe e das memórias | Fora do escopo atual: cancelado em 13/11/2026 | Os dois usam o SMBus; risco ao SPD e driver fora do kernel ([decisão](DECISOES.md#iluminação-asus-da-placa-mãe-e-das-memórias-cancelada--13112026), [referência](REFERENCIA_ASUS_AURA.md)) |
| Governador CPU/GPU → fans | Proposto | Resposta e falha de fonte em ensaio controlado |
| Interface (layout CAM, bandeja) | Implementado; Mini Mode retirado a pedido do usuário | Conferir no Ubuntu contra as capturas; prévia Windows permite revisar o layout |
| Instalação, atualização, remoção | Implementado, sem execução no Ubuntu | Atualização em Sobre pelo commit de produção, como no Facecam; conferir publicação, rollback, configuração e reinício seguro no Ubuntu |
| Firmware, conta, telemetria, loja | Fora do escopo | — |
| Outras famílias do CAM e outras marcas | Fora do escopo atual | Exigem hardware para validar |

## Ordem de trabalho

1. **Referência.** Levantar no `index.node` os relatórios HID dos dois dispositivos
   e registrar em [Referência Windows](REFERENCIA_WINDOWS.md).
2. **Fundação.** `engine`, `ipc`, CLI, `config`, `devices` (somente leitura) e
   `sensors`, com build headless e testes de contrato.
3. **Leitura dos dispositivos.** Status do Kraken X e do Smart Device sem escrever.
4. **Escrita segura.** Curva de segurança do Kraken, rotação do Smart Device,
   pisos, leitura de volta e encerramento; depois o governador.
5. **Iluminação.** Modos estáticos, depois os dinâmicos (temperatura, áudio).
6. **Interface.** Layout do CAM, Modo Mini, bandeja, idiomas.
7. **Perfis e distribuição.** Profile Sync, instalação, atualização e remoção.

## O que conta como paridade

Separe presença de controle, comportamento correto no dispositivo, fidelidade
visual, operação contínua e confiabilidade. Uma tela parecida ou todos os botões
desenhados não estabelece equivalência. Cada evidência indica versão, hardware,
cenário, resultado e limitações. Não existe porcentagem global de paridade.
