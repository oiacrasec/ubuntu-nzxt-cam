# Decisões de implementação

Esta página registra por que os caminhos atuais foram escolhidos. Propostas
futuras permanecem separadas dos mecanismos existentes.

## Escopo igual ao CAM — 12/11/2026

**Problema:** o NZXT CAM não tem versão Linux. **Alternativas:** (a) app nativo
com as funções do CAM; (b) rodar a interface Electron do CAM no Linux, trocando só
o núcleo nativo; (c) hub de várias marcas (placa-mãe, memória, placa de som).
**Escolha:** (a). O usuário pediu o porte "as-is" do CAM. (b) dependeria do código
proprietário de cada pessoa, quebraria a cada atualização do CAM e traria Electron;
(c) foi descartado para reduzir o escopo. O Arctis Nova Pro já tem solução Linux.
**Consequência:** só entram recursos do CAM aplicáveis ao Kraken X e ao Smart
Device V1; outras famílias ficam fora do escopo atual por falta de hardware.

## Mesmo nome do produto — 12/11/2026

O usuário escolheu manter o nome **NZXT CAM**, pois o foco é o porte. Binário,
diretório de configuração e regra udev usam `nzxt-cam`. O README mantém o aviso
de projeto independente, sem afiliação com a NZXT. Uma distribuição pública
precisaria rever o uso da marca; isso não afeta o uso pessoal.

## Rust nativo e controlador único

Rust com egui/eframe atende o frontend e ksni a bandeja, como no Facecam Hub.
Isso preserva um build headless pequeno, sem Python, Qt, GTK ou navegador, e
evita duplicar o acesso aos dispositivos em cada interface. Um único processo
controlador também é a garantia de que só um escritor usa cada hidraw.

## Implementação própria sob MIT

O liquidctl já suporta esses dispositivos, mas é GPL-3. Usá-lo como dependência
ou copiar seu código obrigaria o projeto a ser GPL-3. **Escolha:** MIT com
implementação própria; o liquidctl e sua documentação servem como referência de
fatos de protocolo, e a evidência principal vem do CAM Windows.

## Curva de segurança no firmware do Kraken

Um governador em espaço de usuário pode morrer. Gravar primeiro um perfil interno
baseado no líquido faz o próprio Kraken continuar refrigerando sem o aplicativo.
O Smart Device V1 não oferece isso; a limitação fica documentada em
[Arquitetura](ARQUITETURA.md#segurança-térmica).

## Desenvolvimento no Windows, validação no Ubuntu

A análise do CAM acontece na máquina Windows onde ele está instalado. Build,
testes e ensaios com hardware acontecem no Ubuntu de destino, por escolha do
usuário; WSL não é usado para validar hidraw, hwmon ou udev.

## "Fixo" no Kraken é uma curva plana com teto de segurança — 12/11/2026

**Problema:** com fonte CPU ou GPU, o governador escreve rotações calculadas; se
ele morrer, o Kraken ficaria com a última rotação, como o Smart Device.
**Escolha:** no Kraken, toda escrita é uma curva pela temperatura do líquido. Uma
rotação calculada vira uma curva plana nesse valor até 59 °C e 100% a partir de
60 °C. **Consequência:** sem o aplicativo, o próprio firmware ainda acelera bomba e
fan se o líquido esquentar. Ao sair normalmente, o controlador grava a curva do
perfil (fonte Líquido) ou Silent pelo líquido. **Validação:** etapa 6 do
[roteiro](VALIDACAO_LOCAL.md).

## Testes no Windows, produto no Linux — 12/11/2026

O usuário pediu para adiantar no Windows tudo o que não depende do Linux. Os
módulos que usam APIs Unix (`ipc`, GUI, bandeja e hidraw) ficam atrás de
`cfg(unix)`; no Windows, o binário só informa que executa no Linux. Assim,
`cargo test` roda no Windows a lógica de curvas, governador, configuração,
sensores com árvores sintéticas, descoberta, permissões e protocolos, e
`cargo clippy --target x86_64-unknown-linux-gnu` confere o código Linux. Isso não
substitui o [roteiro no Ubuntu](VALIDACAO_LOCAL.md).

## NVML carregada sob demanda

A GPU NVIDIA é lida pela NVML do driver (`nvml-wrapper`, recurso `nvidia`), que
carrega `libnvidia-ml.so` na primeira leitura. Sem driver NVIDIA, a GPU fica
ausente sem erro; o build não exige CUDA. `nvidia-smi` por processo a cada ciclo
foi descartado pelo custo de criar um processo por segundo. GPUs AMD usam hwmon.

## Interface igual ao CAM, com arte própria — 12/11/2026

**Problema:** o usuário quer a interface igual à do CAM. **Referência:** capturas
da janela do CAM 4.76.8 (somente a janela, com autorização do usuário) e o tema
`lib/colors.ts` do renderer. **Escolha:** reproduzir estrutura, ordem das telas,
posição dos elementos, paleta (fundo `(26,28,30)`, menu `(18,18,20)`, bordas
`(52,56,60)`, controles `(73,72,81)`, texto `(232,235,237)`, roxo `(100,10,146)`,
orquídea `(191,76,191)`), tamanhos e rótulos. Ícones são desenhos próprios em
código. Por autorização expressa em 13/11/2026, os ícones originais do aplicativo
e da bandeja e a logo do cabeçalho são a exceção documentada em
[assets/README.md](../assets/README.md).
Ilustrações e as fontes licenciadas pela NZXT (Akkurat, Gotham) continuam fora
da distribuição. **Fora da interface, a pedido do
usuário:** Áudio, Suporte e Beta. **Sem equivalente no hardware ou no porte:**
Privacidade (o porte não coleta dados), Graphics API (tela LCD), Serial Devices.
**Atualização após a revisão do usuário:** Profile Sync e edição por LED foram
implementados, sem validação no Ubuntu. Mini Mode foi retirado da interface;
seus campos antigos continuam legíveis para preservar configurações existentes.

## Atualização em Sobre pelo commit de produção — 12/11/2026

O usuário pediu o fluxo do [Facecam Hub](https://github.com/oiacrasec/ubuntu-elgato-facecam),
consultado na revisão `d32e400` (`src/updater.rs`, `src/installation.rs`, `build.rs`
e tela Sobre). A consulta inicial a releases numeradas foi substituída por
`git ls-remote --symref … HEAD`, aceitando somente a branch padrão main/master.

O worker fixa o commit selecionado, verifica ancestralidade, compila com os mesmos
recursos, confere `--build-info` e só então chama o instalador local existente.
O registro guarda prefixo, revisão, recursos e SHA-256. A publicação verifica o
checksum e conserva backup do executável/registro para restaurar em caso de falha.
Ao contrário do bundle de recursos da Facecam, este porte usa sua instalação atual
de binário, ícone e atalho; não há bundle de fundos. Os dois ícones originais
autorizados são instalados conforme a nota de origem em `assets/README.md`.

O controlador continua ativo durante download/compilação. Reiniciar é uma ação
separada e depende de preparar a saída térmica dos canais sem erro. A nova instância
espera a anterior sair antes de adquirir o controle. Na prévia Windows, todo esse
fluxo é simulado sem rede nem alterações em arquivos. Instalação, rollback e reinício
reais continuam pendentes de ensaio no Ubuntu.

## Iluminação ASUS da placa-mãe e das memórias cancelada — 13/11/2026

**Problema:** o usuário pediu os LEDs da ROG MAXIMUS XI HERO e das memórias
G.Skill na página **Iluminação**. **Contexto:** a análise confirmou que os dois
usam o SMBus ([referência](REFERENCIA_ASUS_AURA.md)). A placa-mãe fica no SMBus
do Super I/O, que no Ubuntu exige o `i2c-nct6775` com patch fora do kernel
oficial; as memórias exigem varrer o barramento e reendereçar os controladores no
mesmo barramento do SPD, onde um erro pode inutilizar um pente.
**Alternativas:** (a) implementação própria a partir dos fatos documentados,
mantendo MIT, com auxiliar privilegiado, lista fechada de endereços e backup do
SPD; (b) copiar o código do OpenRGB, o que torna o projeto GPL-2.0 e traz a
operação arriscada para dentro do nosso binário; (c) delegar ao OpenRGB pelo SDK
local (porta `6742`), sem código nosso no SMBus e sem mudar a licença.
**Escolha:** nenhuma; o usuário cancelou o plano. O risco ao hardware e a
dependência de um driver fora do kernel não compensam neste momento.
**Consequência:** placa-mãe e memórias ficam fora do escopo atual. A referência
fica guardada; se o tema voltar, (c) é o caminho recomendado e (b) só com a
decisão explícita de relicenciar o projeto. A placa de som AE-5 Plus segue como
proposta separada ([referência](REFERENCIA_SOUND_BLASTER.md)).

## Como registrar uma nova decisão

Indicar data, problema, contexto, alternativas examinadas, escolha, consequências
e forma de validação. Não descrever arquitetura futura como se o código já a executasse.
