# Orientações para agentes e contribuidores

Leia [CLAUDE.md](CLAUDE.md), o [manual](docs/README.md) e os
[padrões de documentação e escrita](docs/PADROES_DE_DOCUMENTACAO.md).
Os contratos de arquitetura e os limites documentados se aplicam a GUI, CLI e testes.

Implemente ações operacionais no controlador; preserve o build headless e o código
Rust sem `unsafe`. As regras de segurança térmica de [CLAUDE.md](CLAUDE.md) têm
precedência sobre paridade visual ou conveniência.

Não copie código do liquidctl nem recursos proprietários do NZXT CAM. Não declare
como implementado um recurso que só existe na referência Windows. Testes com
hardware são optativos e não devem escrever em dispositivos reais nos checks comuns.
