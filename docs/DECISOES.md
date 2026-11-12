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

## Como registrar uma nova decisão

Indicar data, problema, contexto, alternativas examinadas, escolha, consequências
e forma de validação. Não descrever arquitetura futura como se o código já a executasse.
