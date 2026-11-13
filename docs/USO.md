# Uso do NZXT CAM para Ubuntu

Compile, instale, prepare as permissões e abra o aplicativo pelo menu.
Esta página descreve o comportamento implementado; o estado de validação de cada
item está no [roadmap](ROADMAP.md).

## Conferir o layout no Windows

**Implementado:** o modo `--preview` abre a mesma interface egui com dados
simulados de CPU, GPU, Kraken X e Smart Device V1. Serve para conferir layout,
navegação, editores e Profile Sync; não valida funcionamento de hardware no Ubuntu.

No PowerShell, na raiz do repositório, com Rust instalado:

```powershell
cargo build --no-default-features --features gui
.\target\debug\nzxt-cam.exe --preview
```

Depois de compilar, também é possível abrir `preview-windows.cmd` com duplo clique.
A janela tem o título **NZXT CAM — Prévia (dados simulados)**. Os números são
exemplos fixos, não leituras do computador. Mudanças de iluminação, curvas e
preferências ficam apenas na memória e são descartadas ao fechar. A prévia não
inicia o controlador Linux, não abre dispositivos, não lê nem salva configurações
reais, e recusa importação, exportação, início automático e operações de permissões.
`--preview` não pode ser combinado com outras opções. Sem ele, o build Windows
continua sem controle operacional do hardware.

## Compilar e instalar

```bash
git clone https://github.com/oiacrasec/ubuntu-nzxt-cam.git
cd ubuntu-nzxt-cam
sudo apt install build-essential pkg-config libxkbcommon-dev libwayland-dev \
  libx11-dev libgl1-mesa-dev libdbus-1-dev pulseaudio-utils
export PATH="$HOME/.cargo/bin:$PATH"
./test-desktop-app.sh
./install-desktop-app.sh            # ~/.local por padrão
./install-desktop-app.sh --autostart  # opcional: iniciar com a sessão
```

Use Rust 1.95 ou posterior ([instalação](https://rustup.rs/)). O instalador copia
`nzxt-cam` para `~/.local/bin`, cria o atalho do menu e o ícone. `./uninstall-desktop-app.sh`
remove o aplicativo e preserva a configuração; `--purge` também apaga a configuração.

## Preparar o acesso aos dispositivos

Sem preparação, `/dev/hidraw*` pertence ao root e o dispositivo aparece como
**Sem permissão de acesso**.

1. Abra **Configurações** › **Acesso aos dispositivos** › **Preparar permissões…**.
2. Revise os comandos e o conteúdo da regra exibidos. Nada foi alterado ainda.
3. Clique em **Confirmar e autenticar** e informe a senha na janela do Ubuntu.
4. Ao terminar, o aplicativo reabre os dispositivos; o estado passa a **Pronto**.

A regra `/etc/udev/rules.d/71-nzxt-cam.rules` concede à sessão ativa somente o
Kraken X (`1e71:170e`) e o Smart Device V1 (`1e71:1714`). **Remover regra…** desfaz.
Pela CLI: `nzxt-cam --prepare-permissions`, depois `nzxt-cam --confirm-plan ID`.

## Telas

A janela segue o layout do NZXT CAM: barra superior, menu lateral e cartões.

- **PC Monitoring**: CPU e GPU (carga, temperatura, clock, ventilador), RAM, rede,
  armazenamento por disco montado e principais processos.
- **Especificações do sistema**: CPU, GPU, placa-mãe, RAM, armazenamento, áudio e rede,
  com itens clicáveis e detalhes do item selecionado ao lado. Campos indisponíveis
  aparecem como `—`. Sem acesso a dados de módulos individuais, a RAM mostra o total.
- **Iluminação**: por canal, brilho e modo, com os mesmos menus do CAM; clicar na
  linha abre cores e as opções do modo (velocidade, direção, tamanho do LED,
  movimento). Vários canais podem ficar abertos; cada linha fecha apenas seu
  próprio detalhe. **Personalizar cores LED** aceita Fixo e Respirar no Anel e na Fita.
  Os modos de áudio (Nivelado, Sincronização, Graves e Ganho) reagem ao som que
  o computador toca e precisam do `parec`: `sudo apt install pulseaudio-utils`.
- **Resfriando**: temperaturas de CPU, GPU e líquido; por canal, RPM, fonte
  (**Líquido**, **CPU**, **GPU**) e perfil **Silencioso**, **Desempenho**, **Fixo** ou
  **Personalizado**. Clicar na linha abre o editor de curva.
- **Configurações**: **Geral** (idioma, °C/°F, temperatura da CPU, bandeja, início,
  backup em `~/nzxt-cam-backup.json` e painéis visíveis), **NZXT Firmware**,
  **Acesso aos dispositivos**, **Profile Sync** e **Sobre** (atualizações do aplicativo).

O Mini Mode foi retirado da interface a pedido do usuário; campos antigos da
configuração continuam legíveis. Com a bandeja disponível, fechar a janela mantém o governador
ativo; **Sair** na bandeja encerra o aplicativo. No GNOME, a bandeja pode exigir a
extensão AppIndicator.

Na tabela de processos, os cabeçalhos ordenam as linhas e a seta indica a direção.
GPU, download e upload por processo ficam em `—` quando não há fonte de leitura;
o coletor Linux atual fornece CPU e RAM por processo e tráfego agregado de rede.

## Profile Sync

**Implementado, sem validação no Ubuntu.** Em **Configurações** › **Profile Sync**:

1. Adicione perfis nos seletores das páginas **Iluminação** e **Resfriando**.
   Os ajustes de canais são salvos no perfil associado ao mestre ativo.
2. No seletor de **Profile Sync**, **Adicionar perfil** abre o diálogo de nome.
   Salvar cria e ativa o perfil; a engrenagem permite renomeá-lo ou excluí-lo.
3. Selecione os perfis de iluminação e refrigeração. A seleção aplica imediatamente;
   o seletor **Perfil ativo** da barra superior também ativa perfis salvos.
4. Configure a ativação por programa ou horário nos respectivos diálogos.
   Programas aceitam múltiplos executáveis, pesquisa local e seleção por arquivo.
   O catálogo local do Ubuntu não reproduz o catálogo de jogos da NZXT.
   Horários têm dias da semana, início incluído, fim excluído e **Dia inteiro**.
   Um intervalo noturno pertence ao dia em que começa. Apenas 00:00–00:00
   representa o dia inteiro; outros horários iguais representam intervalo vazio.

`Default` não aceita ativação automática nem remoção. As regras são verificadas
a cada três segundos. Cada nova ativação entra em uma pilha; quando uma regra
termina, a seleção anterior é retomada. Seleções
manuais não são reaplicadas continuamente pela mesma regra: uma nova transição
é necessária. Na prévia Windows, a ativação é manual e a avaliação automática
não consulta os processos nem o relógio do computador.

O controlador valida todos os canais antes de escrever; um canal indisponível
recusa a ativação. Uma falha durante o envio pode deixar parte da iluminação
aplicada e é informada; o perfil não é declarado ativo nesse caso. Os pisos,
fontes e limites térmicos continuam valendo para a refrigeração de qualquer perfil.

Os ícones originais do CAM são usados na bandeja, janela e lançador do Ubuntu,
por autorização do usuário. Origem e direitos estão em [assets/README.md](../assets/README.md).

## Atualizar o aplicativo

**Implementado, sem ensaio de instalação no Ubuntu.** A atualização segue o
fluxo do Facecam Hub: consulta o commit da branch padrão `main` ou `master` do
repositório do projeto, sem depender de releases numeradas.

1. Instale pelo `install-desktop-app.sh` para registrar prefixo, build e checksum.
   Para uma instalação anterior sem registro, execute novamente o instalador.
2. Em **Configurações** › **Sobre**, clique em **Verificar atualizações**.
3. Quando houver um candidato, clique em **Atualizar agora**. Git, Rust/Cargo e
   as dependências de compilação do aplicativo precisam estar disponíveis.
4. Ao concluir, clique em **Reiniciar**. A instalação não reinicia por conta própria.

Download e compilação rodam em segundo plano; o controlador continua refrigerando.
Antes de publicar, a atualização confere a revisão selecionada, sua continuidade
em relação à instalação, a identidade do novo binário e os recursos GUI/bandeja/NVIDIA.
A troca usa o instalador existente, preserva a configuração e verifica o checksum
do resultado; falhas na publicação tentam restaurar o executável e o registro
anteriores. Falhas e o caminho do log aparecem em **Sobre**. O reinício só prossegue
se o controlador conseguir preparar o estado térmico de saída dos canais.

Os arquivos de atualização ficam em `~/.cache/nzxt-cam/updates`, respeitando
`XDG_CACHE_HOME`; o registro fica ao lado do binário instalado, em
`nzxt-cam.installation.json`. `nzxt-cam --build-info` mostra a identidade do build.
Na prévia Windows, os três botões apenas simulam estados, sem rede, instalação ou reinício.

## Segurança térmica

- Toda curva precisa chegar a 100% na temperatura crítica da fonte: 60 °C para o
  líquido, 90 °C para CPU e GPU. A interface recusa curvas que não cheguem.
- Uma fonte indisponível leva o canal a 100% e mostra **Segurança** no canal.
- No Kraken X, a curva com fonte **Líquido** roda no próprio dispositivo. Com fonte
  CPU ou GPU, o aplicativo grava uma curva plana que ainda sobe a 100% com o
  líquido a 60 °C, mesmo que o aplicativo pare.
- O Smart Device V1 não tem curva interna. Ao sair normalmente, os fans vão para a
  rotação de saída (padrão 100%). Se o processo morrer abruptamente, fica a última
  rotação gravada: é uma limitação conhecida.

## Linha de comando

```bash
nzxt-cam --list-devices      # descoberta, sem abrir dispositivos
nzxt-cam --sensors           # uma leitura de sensores em JSON
nzxt-cam --headless          # controlador sem janela
nzxt-cam --status            # estado da instância em execução
nzxt-cam --set-cooling 'kraken_x:SERIAL/pump' perfil.json
nzxt-cam --set-lighting 'kraken_x:SERIAL/ring' luz.json
nzxt-cam --quit
```

O identificador do canal aparece em `--status`, em `devices[].info.id`, seguido de
`/` e do nome do canal (`pump`, `fan`, `fan1`…). Exemplos de arquivo:

```json
{"profile": {"type": "custom", "points": [
  {"temperature": 30, "duty": 40}, {"temperature": 45, "duty": 70},
  {"temperature": 60, "duty": 100}]}, "source": "liquid"}
```

```json
{"mode": "breathing", "colors": [[126, 34, 206]], "speed": "slow"}
```

## Configuração

O arquivo fica em `~/.config/nzxt-cam/config.json`, respeitando `XDG_CONFIG_HOME`;
`--config ARQUIVO` seleciona outro. Campos desconhecidos na raiz são preservados,
um JSON inválido permanece intacto e o primeiro salvamento guarda
`config.json.backup`. Traduções da comunidade podem ser colocadas em
`~/.config/nzxt-cam/locales/<idioma>.json`, com as mesmas chaves de
[`locales/en-US.json`](../locales/en-US.json).
