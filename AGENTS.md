# ESPECIFICAÇÃO TÉCNICA: COMPILADOR DE EXPRESSÕES EM RUST

## [0] CONTINUIDADE ENTRE SESSÕES
- **Antes de qualquer outra coisa, leia @./ESTADO_DO_PROJETO.md.** Ele diz em que fase o projeto
  está agora e qual é a próxima ação concreta — este `AGENTS.md` é o processo fixo a seguir, mas
  não muda para refletir progresso; `ESTADO_DO_PROJETO.md` é o estado atual, e deve ser mantido
  atualizado a cada etapa concluída.

## [1] ESCOPO DA LINGUAGEM
- Tipo: Linguagem baseada estritamente em expressões.
- Arquitetura: Lexer -> Parser (AST) -> Codegen (Geração de bytecode para uma pilha virtual simples ou avaliação direta).
- A especificação detalhada, com a gramática em EBNF, está em @./xpr-specs.md (ver
  @./ESTADO_DO_PROJETO.md para como essa arquitetura resumida acima se traduz nas decisões reais
  já tomadas — por exemplo, Codegen aqui significa gerar Rust e compilar com `rustc`/`cargo`, não
  bytecode).

## [2] DIRETRIZES DE IMPLEMENTAÇÃO E FLUXO DE TRABALHO
Você deve seguir estritamente o fluxo sequencial abaixo para cada etapa (Lexer, Parser, Codegen).

### Fase 1: Planejamento
1. Planeje as etapas de implementação: lexer, parser, codegen. Divida em subetapas, se necessário. Para isso:
   1.1. Gere os arquivos plano-lexer.md, plano-parser.md, plano-codegen.md
   1.2. Cada arquivo conterá o esboço da API necessárias para a sua implementação, as estruturas 
2. Planeje o ambiente:
   2.1. Gere o ambiente de desenvolvimento Rust (`cargo init`)
   2.2. Gere os casos de teste automatizados em Rust (usando `#[test]`) para a etapa atual antes de codificar a lógica (TDD).
   2.3. Configure o arquivo `Cargo.toml`.
   2.4. Crie os scripts ou comandos de teste necessários (`cargo test`).
   2.5. Inicialize o ambiente de versionamento Git se não estiver inicializado.

### Fase 2: Execução Incremental
Para CADA etapa planejada, execute:
1. Gere o código estrito da etapa atual.
2. Execute os testes (`cargo test`).
3. Corrija eventuais erros de compilação ou lógica baseando-se estritamente no output do terminal.
  3.a. A etapa SÓ é considerada completa se todos os testes daquela etapa específica derem VERDE (passarem).
  3.3.1. CRÍTICO: Se houver um bug ou erro *no arquivo de testes*, faça um relatório e o usuário irá intervir; caso contrário deve ajustar a lógica de código para passar no teste original. JAMAIS altere ou gere novos arquivos de teste para mascarar um comportamento incorreto do compilador. Os testes guiam o código, e não o contrário.

### Fase 3: Fechamento de Etapa (Etapa 3.4 da solicitação)
Ao final de cada etapa (Lexer concluído, Parser concluído, Codegen concluído):
1. Crie ou atualize um arquivo chamado `RELATORIO_PROGRESSO.md`. Adicione uma seção para a etapa atual detalhando:
       - Atividades realizadas.
       - Decisões de design/arquitetura tomadas.
       - Resultado final dos testes (Cole o output do sucesso do `cargo test`).
2. Execute um commit no Git com uma mensagem clara descrevendo a conclusão da etapa.

## [3] CONCLUSÃO DO PROCESSO
1. Ao final de todas as etapas (após o Codegen estar funcional e testado), gere um arquivo final chamado `RELATORIO_FINAL.md`. Este documento deve consolidar toda a arquitetura da linguagem, lições aprendidas e o status final do projeto.


