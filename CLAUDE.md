# Orientações do projeto

Aplicação nativa Rust para Ubuntu 24.04+ que porta o NZXT CAM Windows, com o
mesmo nome de produto (**NZXT CAM**); binário, configuração e regra udev usam `nzxt-cam`.
O alvo de paridade funcional e visual é **NZXT CAM 4.76.8.70**, no escopo do
hardware disponível: Kraken X (X52/X62/X72, `1e71:170e`) e Smart Device V1
(`1e71:1714`). Outras marcas e outras famílias NZXT ficam fora do escopo atual.
Leia [a arquitetura](docs/ARQUITETURA.md) antes de alterar o controlador,
o governador de refrigeração ou o acesso aos dispositivos.
O [manual](docs/README.md) organiza uso, desenvolvimento, referência e decisões.
Para documentação, mensagens e rótulos, siga os
[padrões de documentação e escrita](docs/PADROES_DE_DOCUMENTACAO.md).

Não adicionar Python, Qt, GTK, Electron ou webview ao produto. A interface do CAM
é Electron; o porte reproduz funções e layout em egui, sem reutilizar o código
da NZXT. O caminho escolhido é análise estática do CAM Windows e implementação
própria em Rust. O liquidctl (GPL-3) serve somente como referência de fatos de
protocolo; não copiar seu código para este projeto MIT.
[Referência Windows](docs/REFERENCIA_WINDOWS.md) registra hashes, componentes,
recursos observados e limites da comparação.

Compilação e testes acontecem no Ubuntu de destino. A máquina Windows da
referência serve à análise do CAM; não presumir que um build Windows ou WSL
valide hidraw, hwmon ou udev.

## Regras de segurança do hardware

- Refrigeração é crítica. Nenhuma falha pode deixar bomba ou fans parados ou presos
  em rotação baixa sem aviso. Ver [Arquitetura](docs/ARQUITETURA.md#segurança-térmica).
- Toda escrita valida canal, faixa e piso mínimo antes de enviar e relê o estado
  quando o dispositivo permitir. A bomba tem piso próprio, maior que o dos fans.
- Uma fonte de temperatura indisponível leva o canal ao valor de segurança,
  nunca ao último valor calculado.
- Não atualizar firmware. Não enviar comandos não documentados na referência.
- Testes comuns não escrevem em dispositivos reais. [Testes](docs/TESTES.md) separa
  verificações automáticas das que exigem hardware.

## Contratos de código

- Um único controlador (`engine.rs`) atende GUI, bandeja e CLI; interfaces só
  enviam pedidos e exibem o snapshot.
- `--no-default-features` deve continuar funcional sem GUI e bandeja.
- Mantenha `unsafe_code = "forbid"` no código da aplicação.
- Dependências de sistema usam APT. Operações administrativas (regra udev) mostram
  o plano e pedem confirmação antes de autenticar por `pkexec`; não armazenar senhas.

`package/` contém a cópia do CAM instalado e evidências locais, ignorados pelo Git.
Não copiar recursos proprietários do CAM (ícones, imagens, traduções) para a
distribuição sem autorização expressa do usuário registrada aqui.

Em 13/11/2026 o usuário autorizou expressamente usar a logo/ícone original do
NZXT CAM na bandeja e no sistema. A autorização abrange os dois arquivos de
ícone em `assets/`, cuja origem está em `assets/README.md`; não transfere direitos
da NZXT nem abrange outros recursos proprietários.

No mesmo dia, o usuário autorizou também a logo NZXT CAM no canto superior
esquerdo da interface: `assets/nzxt-cam-wordmark.svg` e sua rasterização PNG.
