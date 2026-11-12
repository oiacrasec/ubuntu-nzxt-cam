# Padrões de documentação e escrita

Escreva para ajudar a pessoa a concluir uma tarefa ou entender uma decisão.
Comece pela resposta e confirme no código e na aplicação o comportamento descrito.
Vale para documentação, textos da interface e contribuições.

## Organização

Cada guia responde a uma pergunta. Nas duas primeiras linhas, apresente a resposta
ou a ação principal. Use um título de nível 1 por arquivo e seções de nível 2.
Use listas numeradas para passos em sequência e tabelas para comparar parâmetros.

| Assunto | Documento responsável |
| --- | --- |
| Primeira execução e escopo | [README](../README.md) |
| Camadas, comandos, estado e segurança térmica | [Arquitetura](ARQUITETURA.md) |
| Evidências do CAM Windows | [Referência Windows](REFERENCIA_WINDOWS.md) |
| Estado e próximos passos | [Roadmap](ROADMAP.md) |
| Por que cada caminho foi escolhido | [Decisões](DECISOES.md) |
| Como reproduzir as verificações | [Testes](TESTES.md) |

Atualize o documento responsável e aponte para ele a partir dos demais. Use links
relativos e, ao renomear uma página, atualize quem aponta para ela.

## Linguagem e nomenclatura

Use português direto, voz ativa e verbos concretos. Evite adjetivos promocionais,
"basta", "simplesmente" e promessas sem verificação. O produto é **NZXT CAM**, como
o original; quando houver ambiguidade, escreva **NZXT CAM Windows** para a referência
e **NZXT CAM para Ubuntu** para o porte. O binário e os caminhos usam `nzxt-cam`.
Identificadores técnicos mantêm a grafia do código.
Reproduza rótulos da interface em **negrito**, e use crases para comandos,
caminhos, parâmetros e valores.

Informe a unidade real de cada leitura: RPM, duty em %, °C, dBA. Não chame um
duty de RPM, nem uma rotação calculada de rotação medida.

## Afirmações e evidências

Descreva recursos com seu estado explícito: Implementado, Limitação conhecida,
Proposto ou Fora do escopo atual, conforme o [manual](README.md). Uma opção
visível ou um build concluído não comprovam funcionamento no dispositivo; texto
presente no CAM não comprova que o recurso se aplica a um modelo.

Em relatórios, registre data, versão ou revisão, ambiente, procedimento e resultado.
Marque como não verificado o que não foi exercitado. Ao citar uma API ou ferramenta
externa, mantenha o link para a documentação oficial junto da afirmação.

## Mensagens

Uma mensagem de erro identifica a operação que falhou e uma ação possível.
Falhas de refrigeração são sempre visíveis; não declare sucesso após uma falha.
Estados vazios explicam o próximo passo, como conectar o dispositivo ou preparar
as permissões.
