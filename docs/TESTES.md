# Testes

**Implementado:** testes de curvas, governador, configuração, sensores sintéticos,
descoberta, permissões, protocolos, Profile Sync, atualização e isolamento da prévia.
Execução real, IPC e integração desktop no Ubuntu continuam pendentes. A lista
abaixo reúne a cobertura existente e os critérios para completar a validação.

## Verificações automáticas

Rodam em qualquer Ubuntu, sem dispositivo, e nunca escrevem em hardware real.

- Curvas: pontos negativos, zero, máximos, fora de ordem, decrescentes, não finitos,
  abaixo do piso; interpolação e histerese.
- Iluminação: modos inexistentes para o dispositivo, cores e velocidades fora da faixa.
- Protocolo: codificação e decodificação de relatórios contra amostras gravadas,
  incluindo relatórios curtos, truncados e com valores inesperados.
- Sensores: árvores sysfs e `/proc` sintéticas, sensor ausente e leitura velha.
- Governador: fonte indisponível leva ao valor de segurança; encerramento grava o
  valor de saída; um dispositivo simulado registra a sequência de escritas.
- Controlador e IPC: contratos de `Request`/`Reply`, instância única, limite de
  tamanho do pedido e configuração inválida preservada.
- Profile Sync: captura de perfis independentes, nomes/referências inválidas,
  horários que cruzam a meia-noite, nome exato de processo, recusa antes de qualquer
  escrita simulada e persistência do perfil ativo pelo controlador.
- Atualização: branch padrão `main`/`master`, SHA completo, identidade e recursos
  do build, fases de erro/reinício e consulta a um repositório Git temporário local.
  O teste de cancelamento do grupo de processos exige Unix. Nenhum teste busca
  atualizações na rede, publica binário instalado ou reinicia o aplicativo real.
- Prévia Windows: alterações só no snapshot em memória, recusa de E/S no transporte
  e bloqueio de operações de arquivos/permissões/início automático.

## Verificações com hardware

Optativas, com o dispositivo conectado e confirmação explícita. Cada execução
registra data, revisão, modelo, firmware e resultado em uma página de validação.

- Status do Kraken X e do Smart Device V1 como usuário comum após a regra udev.
- Curva de segurança do Kraken: matar o processo e confirmar que a bomba segue o líquido.
- Pisos, leitura de volta e reaplicação após suspensão.
- Modos de iluminação comparados com o CAM no Windows.
