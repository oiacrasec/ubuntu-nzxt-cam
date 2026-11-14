# Manual do NZXT CAM para Ubuntu

O projeto porta o NZXT CAM para Ubuntu 24.04+ com uma aplicação nativa Rust.
Comece pela [arquitetura](ARQUITETURA.md) e pela [referência Windows](REFERENCIA_WINDOWS.md).

## Estado das informações

**Implementado** significa comportamento presente no código e verificável pelos
checks indicados. **Limitação conhecida** descreve algo que o produto ainda não
resolve. **Proposto** identifica um desenho técnico sem implementação. **Fora do
escopo atual** mantém registrado um tema sem prometer sua entrega.

Em 12/11/2026 o código existe e passa nos testes automáticos, mas ainda não foi
executado no Ubuntu: "implementado" significa presente no código, não validado no
hardware. As observações do CAM são evidências da referência Windows, não do porte.

## Uso

- [Compilar, instalar, permissões, telas e CLI](USO.md)
- [Roteiro de validação no Ubuntu e resultados](VALIDACAO_LOCAL.md)
- [Configuração de referência e como repeti-la no CAM Windows](CONFIGURACAO_DE_REFERENCIA.md)

## Engenharia

- [Arquitetura, controlador e segurança térmica](ARQUITETURA.md)
- [Desenvolvimento no Ubuntu](DESENVOLVIMENTO.md)
- [Testes e verificações com hardware](TESTES.md)
- [Padrões de documentação e escrita](PADROES_DE_DOCUMENTACAO.md)

## Planejamento e evidências

- [Roadmap, paridade e critérios de prontidão](ROADMAP.md)
- [Ponto de retomada e próximos passos](PLANO_DE_CONVERSAO.md)
- [Decisões de implementação](DECISOES.md)
- [Referência Windows: NZXT CAM 4.76.8](REFERENCIA_WINDOWS.md)
- [Possíveis conflitos do CAM Windows com outros softwares](REFERENCIA_WINDOWS.md#possíveis-conflitos-com-outros-softwares)
- [Referência: iluminação da Sound BlasterX AE-5 Plus](REFERENCIA_SOUND_BLASTER.md)
- [Referência: iluminação ASUS Aura da placa-mãe e das memórias](REFERENCIA_ASUS_AURA.md)
