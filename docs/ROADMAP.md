# Roadmap, paridade e prontidão

O alvo é paridade funcional e visual com NZXT CAM 4.76.8.70 para o Kraken X
(X52/X62/X72) e o Smart Device V1, em Rust para Ubuntu 24.04+. Esta página
distingue código existente, evidência, lacunas e condições de aceitação.

## Matriz atual — 12/11/2026

| Dimensão | Estado | Próxima verificação |
| --- | --- | --- |
| Referência Windows | Levantamento inicial | Protocolo HID no `index.node` com Ghidra |
| Controlador, IPC, CLI, configuração | Implementado, sem validação no Ubuntu | Testes unitários no Windows; build e execução no Ubuntu pendentes |
| Permissões (regra udev) | Implementado, sem validação no Ubuntu | Acesso ao hidraw como usuário comum |
| Sensores (CPU, GPU, RAM, disco, rede) | Implementado, sem validação no Ubuntu | Leitura no i9-9900K e na RTX 4070 SUPER |
| Governador e curvas | Implementado, sem validação no Ubuntu | Lógica testada sem hardware; resposta real pendente |
| Kraken X: status | Proposto | Temperatura do líquido, RPM da bomba e do fan |
| Kraken X: curva de segurança e perfis | Proposto | Perfil interno sobrevive ao encerramento do app |
| Kraken X: iluminação Ring/Logo | Proposto | Modos habilitados pelo CAM para este modelo |
| Smart Device V1: fans e ruído | Proposto | RPM, duty e leitura de ruído |
| Smart Device V1: iluminação | Proposto | Modos e acessórios detectados |
| Governador CPU/GPU → fans | Proposto | Resposta e falha de fonte em ensaio controlado |
| Profile Sync | Proposto | Ativação por programa e por horário |
| Interface (layout CAM, Modo Mini, bandeja) | Proposto | Comparação visual com o CAM |
| Instalação, atualização, remoção | Proposto | Prefixo do usuário e preservação de dados |
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
