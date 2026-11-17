# Instruções de trabalho — compilador XPR

## Continuidade e fontes de verdade

1. Antes de qualquer outra coisa, leia `MEMORY.md` inteiro: estado atual, arquitetura,
   lições e próxima ação. Atualize-o a cada etapa ou decisão concluída, editando o estado
   existente em vez de acumular relatos históricos.
2. `xpr-specs.md` é a especificação canônica da linguagem, incluindo EBNF.
3. O compilador segue Lexer → Parser (AST) → checagem de tipos → emissão de Rust → `rustc`.
   O runtime é embutido no Rust gerado; consulte as decisões em `MEMORY.md`.
4. Manter somente estes três documentos de projeto: `AGENTS.md` (processo), `MEMORY.md`
   (continuidade e lições) e `xpr-specs.md` (linguagem). Planos de trabalho ficam na conversa;
   histórico e resultados antigos ficam no Git. Não recriar planos e relatórios separados.

## Implementação incremental e TDD

1. Planeje a etapa atual e as subetapas necessárias antes de implementar.
2. Escreva os testes automatizados Rust (`#[test]`) da etapa antes da lógica e confirme
   que falham pelo comportamento que será implementado. Cubra casos válidos, inválidos e limites.
3. Implemente apenas a etapa atual; execute os testes e corrija a lógica a partir do output.
4. **Regra 3.3.1:** se houver bug ou erro no teste, reporte ao usuário para intervenção.
   Não altere testes ou gere novos testes para mascarar comportamento incorreto do compilador.
   Os testes guiam o código, não o contrário. Mudança de expectativa exige decisão explícita.
5. Para cada comportamento observável novo, acrescente teste de ponta a ponta ou milestone.
   Preserve os casos anteriores; fixtures e milestones não devem ser relaxados para esconder bugs.
6. Execute `cargo test --workspace` e o gate `./scripts/run-all-tests.sh` para concluir
   mudanças funcionais. Uma etapa só está concluída com todos os testes pertinentes verdes.
   Para manutenção exclusivamente documental, confira links, referências e o diff.

## Fechamento

1. Atualize `MEMORY.md` com estado, decisões, lições, pendências e resultado da validação.
   Não copie logs extensos; registre comando e resultado de forma sucinta.
2. Faça um commit com mensagem clara ao concluir uma etapa.
3. Se uma decisão existente parecer errada ou a spec se contradisser, reporte a divergência
   ao usuário antes de reverter a decisão ou mudar o comportamento contratado.
