# Manual do NZXT CAM para Ubuntu

O projeto porta o NZXT CAM para Ubuntu 24.04+ com uma aplicação nativa Rust.
Comece pela [arquitetura](ARQUITETURA.md) e pela [referência Windows](REFERENCIA_WINDOWS.md).

## Estado das informações

**Implementado** significa comportamento presente no código e verificável pelos
checks indicados. **Limitação conhecida** descreve algo que o produto ainda não
resolve. **Proposto** identifica um desenho técnico sem implementação. **Fora do
escopo atual** mantém registrado um tema sem prometer sua entrega.

Em 12/11/2026 não há código: todo o comportamento descrito é **Proposto**, e as
observações do CAM são evidências da referência Windows, não do porte.

## Engenharia

- [Arquitetura, controlador e segurança térmica](ARQUITETURA.md)
- [Desenvolvimento no Ubuntu](DESENVOLVIMENTO.md)
- [Testes e verificações com hardware](TESTES.md)
- [Padrões de documentação e escrita](PADROES_DE_DOCUMENTACAO.md)

## Planejamento e evidências

- [Roadmap, paridade e critérios de prontidão](ROADMAP.md)
- [Decisões de implementação](DECISOES.md)
- [Referência Windows: NZXT CAM 4.76.8](REFERENCIA_WINDOWS.md)
