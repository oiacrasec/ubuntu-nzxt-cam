# Testes

**Estado: Proposto.** Os testes ainda não existem; esta página define o que cada
camada deve cobrir antes de ser declarada implementada.

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

## Verificações com hardware

Optativas, com o dispositivo conectado e confirmação explícita. Cada execução
registra data, revisão, modelo, firmware e resultado em uma página de validação.

- Status do Kraken X e do Smart Device V1 como usuário comum após a regra udev.
- Curva de segurança do Kraken: matar o processo e confirmar que a bomba segue o líquido.
- Pisos, leitura de volta e reaplicação após suspensão.
- Modos de iluminação comparados com o CAM no Windows.
