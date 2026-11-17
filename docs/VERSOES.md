# Versões

Esta página registra o que entrou em cada versão do NZXT CAM para Ubuntu. A
versão em uso aparece em **Sobre** e em `nzxt-cam --build-info`. A
[regra de versão](DESENVOLVIMENTO.md#versão) diz quando o número sobe. Entre
duas versões, cada build é identificado pela revisão do Git.

## 0.2.0 — 17/11/2026

É a primeira versão executada e validada no hardware de referência, o Ubuntu com
Kraken X e Smart Device V1 ([Validação local](VALIDACAO_LOCAL.md#ubuntu--13112026)).
Revisões de `d521dd0` a esta.

**Correções**
- Os fans do Smart Device V1 sem perfil salvo deixaram de ficar presos em
  **Segurança** a 100%. Sem perfil salvo, o canal usava a fonte **Líquido**, que
  esse dispositivo não mede. Agora o padrão é **Silencioso** pela CPU; o Kraken X
  continua pelo líquido (`d521dd0`).

**Funções**
- A tela **Especificações do sistema** passou a mostrar dados que antes
  apareciam como `—`, todos lidos sem root (`e3fae6c`):
  - cada módulo de RAM;
  - o nome do código da CPU, o chipset e o southbridge;
  - nome completo, fabricante e formatos dos discos;
  - formato e canais do áudio;
  - energia da GPU;
  - endereço IP, atualizado a cada 3 s.
- A versão passa a subir a cada conjunto de funções, como no Facecam Hub
  ([decisão](DECISOES.md#versão-do-produto-incrementada-à-mão--17112026)).

**Validado no hardware**
- Regra udev, descoberta, sensores e leitura dos dois dispositivos.
- Curva interna do Kraken e governador dos fans pela CPU.
- Autostart com início minimizado.
- Atualização por **Sobre**, de `81e5b3f` para `d521dd0` e de `d521dd0` para
  `e3fae6c`: a refrigeração continuou ativa durante o build.
- Os 17 itens de **Especificações do sistema**.

**Documentação**
- [Configuração de referência](CONFIGURACAO_DE_REFERENCIA.md) para repetir as
  curvas no CAM Windows.
- Registros sobre iluminação de terceiros: placa de som AE-5 Plus e o plano de
  iluminação ASUS da placa-mãe e das memórias, que foi cancelado
  ([decisão](DECISOES.md#iluminação-asus-da-placa-mãe-e-das-memórias-cancelada--13112026)).

**Limitação conhecida**
- O anel e o logo do Kraken obedecem aos comandos por alguns minutos e depois
  param de responder, no Ubuntu e no Windows. A causa não foi identificada
  ([Validação local](VALIDACAO_LOCAL.md#iluminação-do-kraken-x--13112026)).

## 0.1.0 — 12 e 13/11/2026

Porte inicial, de `e096b08` a `81e5b3f`. A versão foi testada só com testes
automáticos e, no Windows, pela prévia; não chegou a rodar no Ubuntu.

- Controlador único para a janela, a bandeja e a linha de comando: sensores,
  governador de refrigeração e segurança térmica. Inclui a curva de segurança no
  firmware do Kraken.
- Drivers do Kraken X (`1e71:170e`) e do Smart Device V1 (`1e71:1714`).
- Interface no layout do CAM, com o catálogo em inglês:
  - **PC Monitoring** e **Especificações do sistema**;
  - **Iluminação**, com os menus do CAM, cor por LED, modos por temperatura,
    modos de áudio e **Onda**;
  - **Resfriando**, com o gráfico da curva;
  - **Configurações**.
- **Profile Sync**: perfis mestre com ativação manual, por programa e por horário.
- Atualização pelo **Sobre** a partir do commit de produção. Scripts de
  instalação, regra udev com confirmação e `pkexec`, e CI.
