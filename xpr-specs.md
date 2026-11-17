# Especificação XPR

XPR é uma linguagem orientada a expressões na qual toda construção retorna um valor.
`xprc` significa xpr compiler, o compilador.

> Compilador em Rust, que traduz o programa para Rust e o compila com `rustc`/`cargo`
> Executável diretamente no terminal
> Máquina atual: Rayzen 7 com 32GB RAM, Zorin OS 18 (ambiente Linux) 

## Conteúdo

* [1. Conceitos Fundamentais](#1-conceitos-fundamentais)

* [2. Tipos](#2-tipos)

* [3. Gramática (EBNF)](#3-gramática-ebnf)

* [4. Semântica](#4-semântica)

  * [Sinais numéricos unários e espaços em branco](#sinais-numéricos-unários-e-espaços-em-branco)

  * [Ordem de avaliação](#ordem-de-avaliação)

  * [4.1 Módulos e import](#41-módulos-e-import)

  * [4.2 Blocos](#42-blocos)

  * [4.3 iif (Ternário)](#43-iif-ternário)

  * [4.4 Laço for](#44-laço-for)

  * [4.5 Funções](#45-funções)

  * [4.6 Escopo](#46-escopo)

  * [4.7 Atribuições](#47-atribuições)

  * [4.8 Valores e cópias](#48-valores-e-cópias)

  * [4.9 Falhas: try e fail](#49-falhas-try-e-fail)

  * [4.10 Tipos de função, anotações e especialização](#410-tipos-de-função-anotações-e-especialização)

* [5. Funções Integradas (Built-in)](#5-funções-integradas-built-in)

  * [5.1 Formatação de números](#51-formatação-de-números)

  * [5.2 Semântica dos operadores](#52-semântica-dos-operadores)

* [6. Constantes Matemáticas](#6-constantes-matemáticas)

* [7. Erros](#7-erros)

* [8. Interface de linha de comando](#8-interface-de-linha-de-comando)

## 1. Conceitos Fundamentais

* **Orientada a Expressões**: Toda instrução é uma expressão. Blocos, `iif` e laços retornam valores.

* **Módulos**: cada arquivo é um módulo e o programa é o módulo fornecido na linha de comando. O código de outros arquivos entra na compilação apenas por meio de `import`, que atribui a cada arquivo seu próprio espaço de nomes; Tudo é resolvido pelo compilador antes da geração de código; não há carregamento em tempo de execução (veja 4.1).

* **Funções de Primeira Classe**: Funções podem ser passadas como argumentos e retornadas como valores.

* **Tipos estáticos**: Todo o programa é analisado sintaticamente e verificado em termos de tipo antes que qualquer código seja gerado; os erros são divididos em erros de tempo de compilação e falhas em tempo de execução (veja as Seções 4.9 e 7).

## 2. Tipos

* **Int**: inteiro de 64 bits com sinal. Um literal sem ponto decimal ou expoente
  (`3`, `10`, `-5`) é um int.

* **Real**: ponto flutuante de precisão dupla (double). Um literal com ponto decimal ou
  um expoente (`.5`, `3.`, `4.0`, `3e0`) é um real. `NaN`, `INF`, `pi` e `E`
  são reals.

* **Aritmética mista**: um int combinado com um real é promovido a real (veja
  5.2). Onde quer que dois tipos sejam unificados (elementos de um literal de lista, ramificações de
  `iif`, atribuições a uma única variável), unificar int com real resulta em real. Assim,
  `x = 2.5; x = 1;` deixa `x` como um real contendo `1.0`, e `x = 1; x = 2.5;` torna
  `x` um real a partir de sua primeira atribuição. Um literal fora do intervalo int não é
  convertido silenciosamente em um real (veja a Seção 3).

* **Number** não é um nome de tipo. A palavra é mantida livre para um possível
  tipo numérico de precisão arbitrária.

* **String**: strings UTF-8, contendo qualquer codepoint Unicode, com sequências de escape (`\n`, `\t`, `\r`, `\0`, `\"`, `\'`, `\\`).

* **Bool**: `true` e `false`, que são constantes predefinidas (veja a Seção 6).

* **List**: coleção ordenada de valores de um único tipo de elemento, `[T]`.
  `[1, 2, 3]` é um `[int]`; `[[1, 2], [3]]` é um `[[int]]` (sublistas podem ter
  tamanhos diferentes); `[1, 2.5]` é um `[real]` contendo `1.0` e `2.5`.

  * O **tipo** de uma lista é apenas o tipo do seu elemento. O **comprimento** é uma
    propriedade do valor: dois valores do tipo `[int]` podem ter comprimentos diferentes,
    e um comprimento pode depender de dados conhecidos apenas quando o programa é executado,
    por exemplo, uma lista construída a partir da entrada de dados.

  * Consequentemente, uma diferença de comprimento entre duas listas não é um erro de tipo.
    Ela é detectada quando a operação executa e gera a falha `size-mismatch`
    (veja 5.2), que pode ser capturada por `try`. O tipo com comprimento fixo é a tupla.

  * Uma lista é um valor imutável: seu comprimento nunca muda. Operações como
    `cat` produzem uma nova lista com um novo comprimento.

* **Escalar versus lista**: um número, string ou bool nunca é uma lista de um único
  elemento. `0` e `[0]` têm tipos diferentes, assim como `""` e `[""]`.

* **Tuple**: coleção ordenada de um número fixo de valores cujos tipos podem
  diferir, escrita com a mesma notação `[val1, val2, ...]`. `[1, "a"]` é a
  tupla `[int, string]`. O comprimento e o tipo de cada posição fazem parte
  do tipo da tupla. Uma tupla cujas posições possuem todas o mesmo tipo é uma lista.

* **Tipo de um literal entre colchetes**: o compilador o infere. Se todos os elementos possuírem
  o mesmo tipo (após a unificação int-para-real), o literal é uma lista;
  caso contrário, é uma tupla. Um literal como `[1, 2]` é sempre uma lista, nunca uma
  tupla. `[]` é uma lista vazia de qualquer tipo de elemento; seu tipo de elemento é
  resolvido pelas outras atribuições e usos da mesma variável. Se nunca for resolvido,
  a variável só poderá conter a lista vazia.

* **Function**: um valor do tipo `A -> R` com um argumento e um resultado (veja
  4.10).

## 3. Gramática (EBNF)

```ebnf
// O arquivo de entrada e cada arquivo importado possuem a mesma estrutura (veja 4.1).
// Itens terminam com ";" e imports devem vir primeiro.
program        = module ;
module         = { import_decl ";" } { definition ";" } ;
import_decl    = "import" "(" string [ "," string ] ")" ;
definition     = identifier "=" initializer ;
initializer    = fn_def | const_expr ;
// const_expr: uma expressão sem fn_call, block, iif_expr, for_expr,
// atribuição ou fn_def, cujos nomes se referem a definições anteriores do mesmo
// módulo ou a nomes qualificados de módulos importados (veja 4.1).

expression     = assignment | fn_def | logical_or | block ;

assignment     = binding_pattern "=" expression ;
fn_def         = "fn" "(" param_pattern ")" block ;
binding_pattern = identifier | list_pattern ;
list_pattern   = "[" [ binding_pattern { "," binding_pattern } ] "]" ;

// Parâmetros podem conter uma anotação de tipo opcional (veja 4.10). Nomes de tipo são
// reconhecidos apenas após ":"; eles não são palavras reservadas.
param_pattern  = ( identifier [ ":" type ] )
               | ( "[" [ param_pattern { "," param_pattern } ] "]" ) ;
type           = "bool" | "int" | "real" | "string" | "fn"
               | "[" "]"                            // qualquer lista
               | "[" type "]"                       // lista de tipo
               | "[" type "," type { "," type } "]" ; // tupla

logical_or     = logical_and { "or" logical_and } ;
logical_and    = logical_not { "and" logical_not } ;
logical_not    = "not" logical_not | equality ;
equality       = relational { ( "==" | "!=" ) relational } ;
relational     = arithmetic { ( "<" | ">" | "<=" | ">=" ) arithmetic } ;
arithmetic     = term { ( "+" | "-" ) term } ;
term           = factor { ( "*" | "/" | "%" ) factor } ;
factor         = [ "+" | "-" ] indexed ;

indexed        = primary { "[" expression { "," expression } "]" } ;
// "v[a, b, c]" é um atalho para "v[a][b][c]" (veja 4.8).

primary        = literal 
               | fn_call 
               | block 
               | iif_expr 
               | for_expr 
               | list_expr
               | try_expr
               | "(" expression ")" ;

literal        = int_lit | real_lit | string | qualified_name ;
// "module_alias.name" é válido apenas quando module_alias foi introduzido por um
// import_decl; o namespace de alias é separado do namespace de variáveis.
qualified_name = identifier [ "." identifier ] ;
fn_call        = qualified_name "(" [ expression ] ")" ;
try_expr       = "try" "(" expression ")" ;
block          = "{" { statement } "}" ;
statement      = ( import_decl | expression ) ";" ;
// import_decl como uma instrução é aceito apenas como uma das primeiras instruções
// do bloco mais externo do programa (veja 4.1).
iif_expr       = "iif" "(" expression "," expression "," expression ")" ;
for_expr       = "for" "(" expression ";" expression ";" expression ")" expression ;
list_expr      = "[" [ expression { "," expression } ] "]" ;

// detectado pelo analisador léxico (lexer)
int_lit   = int_num ;     // fora do intervalo int64: malformed-number
real_lit  = ( int_num ( "." { digit } [ exp_part ] | exp_part ) )
          | ( "." int_num [ exp_part ] ) ;
int_num   = digit { digit } ;
exp_part  = ( "e" | "E" ) [ "+" | "-" ] int_num ;
digit          = ( "0"-"9" ) ;

string         = '"' { chars | escaped } '"' | "'" { chars | escaped } "'" ;
chars          = ? qualquer codepoint Unicode exceto a aspa delimitadora e
                 U+005C ( \ ); codepoints de controle são aceitos, portanto uma string
                 pode abranger várias linhas ? ;
// O lexer apenas delimita strings e não valida seu conteúdo. Codepoints de controle
// são sanitizados quando o texto é exibido em diagnósticos (veja diagnostic.c).
escaped        = "\" ( "n" | "t" | "r" | "0" | '"' | "'" | "\" ) ;

identifier     = ( alfa | "_" ) { alfa | digit | "_" } ;
alfa           = ( "a"-"z" | "A"-"Z" ) ;
// Identificadores contêm apenas ASCII (eles também nomeiam módulos, que são arquivos encontrados por
// nome; veja 4.1). Caracteres não-ASCII são aceitos apenas em strings e comentários.
// Um "." entre dois identificadores é um token separado; um "." imediatamente seguido
// por um dígito ainda inicia um número como ".5".

keywords       = ( "fn", "iif", "for", "try", "import",
                   "and", "or", "not" ) ;
// "true" e "false" são constantes predefinidas (Seção 6), e "bool", "int",
// "real", "string" são nomes que importam apenas em anotações ou como builtins;
// nenhum deles é uma palavra-chave. "number" não é usado: mantido livre para um
// possível tipo numérico de precisão arbitrária.

operators      = ( "+", "-", "*", "/", "%",
                   "<", ">", "=", ">=", "<=", "==", "!=" ) ;
// Reservados para operações bitwise em int, ainda não definidos:
//   "&", "|", "^", "~", "<<", ">>".

// ignorados pelo lexer
comments          = line_comment | multiline_comment ;
line_comment      = "//" { chars } ( "\n" | EOF ) ;
multiline_comment = "/*" { chars } "*/" ;
```

## 4. Semântica

### Sinais numéricos unários e espaços em branco

Quando um `+` ou `-` unário precede um literal numérico, o sinal deve estar diretamente
adjacente a esse literal no código fonte. Espaços em branco, uma nova linha ou um comentário
entre o sinal e o número resultam em um `syntax-error`.

* `2 * -3` e `2 *-3` são válidos.

* `2 *- 3`, `- 3` e `+ 3` são inválidos.

* A subtração binária ainda permite espaços em branco: `2 - 3` é válido.

Esta restrição de adjacência no código fonte complementa a EBNF. Um token de sinal separado
é compatível com a regra: o analisador sintático (parser) pode distinguir o uso unário e binário e verificar as extensões no código fonte. As colunas dos tokens por si só não devem ser confundidas
com deslocamentos de bytes em texto UTF-8.

A gramática existente também admite operandos unários como `-x` e `-(2 + 3)`;
a regra de adjacência de literal numérico acima não redefine essas formas.

### Ordem de avaliação

A precedência determina o agrupamento de expressões. Operandos binários são avaliados da
esquerda para a direita, exceto onde a avaliação de curto-circuito se aplica. Elementos de lista
são avaliados da esquerda para a direita. A atribuição é associativa à direita:
`a = b = expressão` significa `a = (b = expressão)`.

A expressão do argumento é avaliada completamente antes de resolver o alvo da chamada.
Após a avaliação do argumento, a função é resolvida, o valor do argumento é copiado e desestruturado em seus parâmetros, e o corpo é avaliado.
Dessa forma, `f([10, g(20)])` avalia `10`, chama `g(20)`, forma a lista de argumentos,
e só então resolve e chama `f`.

Uma falha em tempo de execução (veja 4.9) abandona a avaliação até o `try` mais interno, ou encerra o programa quando não houver nenhum. A expressão que falhou não produz
resultado. Efeitos colaterais já realizados, incluindo saída de dados, não são desfeitos automaticamente.

### 4.1 Módulos e import

O código de outros arquivos entra na compilação apenas através de `import`. O
compilador localiza, analisa sintaticamente e verifica cada arquivo importado antes de gerar código.
Nada é buscado ou carregado quando o programa compilado executa. Um nome sem
vinculação visível é um `undefined-identifier`; o compilador nunca busca no
sistema de arquivos o alvo de uma chamada.

**Arquivos são módulos.** Existe apenas um tipo de arquivo, o módulo: uma lista de itens
(imports primeiro, seguidos por definições; veja *Conteúdo do módulo*). O arquivo de entrada fornecido
na linha de comando é um módulo como qualquer outro; um programa não é uma
expressão. Um executável é um módulo que define `_main` (veja *Ponto de entrada
`_main`*); um módulo sem `_main` só pode ser importado.

**Caminho de busca (Search path).** `import` localiza arquivos nesta ordem; a primeira correspondência é a utilizada:

1. a biblioteca padrão (a biblioteca privileged carregada quando o compilador
   inicia; seus nomes nunca são ocultados por arquivos do usuário);

2. o diretório do arquivo que contém o `import`;

3. os diretórios fornecidos com `-I`, na ordem da linha de comando.

Não existe variável de ambiente para o caminho de busca: o resultado de uma compilação depende
apenas dos arquivos fonte, da linha de comando e da biblioteca padrão instalada. O
compilador deve ser capaz de relatar o caminho resolvido de cada arquivo que carrega.

**`import("nome")` e `import("nome", "alias")`**

* `nome` é o nome de um módulo sem extensão: `import("tensor")` carrega
  `tensor.x`. `alias` é um identificador; quando omitido, o alias é o próprio `nome`
  (que deve ser um identificador válido). Ambos os argumentos são literais de string.

* Um `import` é aceito apenas no início de um módulo, antes da primeira
  definição. Qualquer `import` posterior é um `syntax-error`.

* Um alias é um **nome de namespace**, mantido em uma tabela separada de variáveis.
  Não é um valor: não pode ser atribuído, passado ou retornado, e
  aparece apenas como a parte esquerda de `alias.membro`. Uma variável e um alias podem,
  portanto, compartilhar um nome. Dois imports com o mesmo alias em um único arquivo geram um
  erro de compilação.

* `alias.membro` é uma expressão cujo valor é a definição pública do módulo
  (uma função ou constante). Pode ser chamada, `alias.f(x)`, ou usada como um
  valor, `g = alias.f;`. Nunca é um alvo válido de atribuição. Um membro desconhecido é um `undefined-identifier`.

* Um módulo é identificado por seu caminho de arquivo canônico. Cada import do mesmo
  arquivo em uma compilação compartilha uma única instância, mesmo sob aliases diferentes.

* Um módulo é **fechado**: ele vê apenas os builtins, suas próprias definições e
  os aliases de seus próprios imports. Ele nunca vê variáveis do arquivo que
  o importa. Isso vale também para o módulo de entrada: suas definições não são
  variáveis no nível de programa.

* Namespaces não são transitivos. Se `tensor` importa `linalg`, um módulo que
  precisa de `linalg` deve importá-lo também.

* Um ciclo de imports gera um erro de compilação.

* Cada módulo XPR é gerado como seu próprio módulo Rust; os membros de um módulo vivem no
  caminho desse módulo (`caminho::nome`) no código gerado. O sistema de módulos do Rust evita por
  construção a colisão entre membros de módulos distintos ou entre um membro e uma variável do
  usuário, sem precisar de um esquema de *mangling* manual. Resta apenas sanitizar, de forma
  injetiva, o caminho de arquivo do módulo (que pode conter caracteres que não são identificadores
  válidos em Rust) para o nome do módulo Rust gerado; esse esquema de sanitização é um detalhe de
  implementação do backend (veja `plano-codegen.md`, subetapa G).

**Conteúdo do módulo**

* Um módulo contém seus imports primeiro, seguidos de suas definições. Expressões livres no nível superior (top-level) não são permitidas.

* Cada definição tem a forma `nome = inicializador`, onde o inicializador é
  uma definição de função ou uma expressão constante. A gramática na Seção 3
  estabelece a restrição para expressões constantes: nenhuma chamada de função, bloco,
  `iif`, `for` ou atribuição fora de corpos de função. Isso garante que o nível superior de um
  módulo seja sempre avaliável em tempo de compilação, sem depender de uma ordem de execução em
  tempo de execução.

* O nível superior de um módulo, portanto, não possui efeitos colaterais e pode ser avaliado pelo
  compilador (por exemplo, dobramento de constantes / constant folding).

* Nomes de nível superior são **definições, não variáveis**. Definir o mesmo nome
  duas vezes é um erro de compilação, e atribuir a um nome de nível superior dentro de qualquer corpo de função é um erro de compilação. Um módulo não possui estado interno mutável: estado é um valor que os chamadores passam e recebem explicitamente, e o estado do sistema (entrada, saída, números aleatórios) pertence aos builtins em tempo de execução.

* As definições seguem a regra de ordem da seção 4.5: uma definição pode usar apenas definições anteriores, além de seu próprio nome pendente para recursão direta. A recursão mútua entre funções de módulo não é possível.

**Nomes privados.** Um nome que começa com `_` é privado para seu módulo. Ele é
usado sem qualificação dentro do módulo, e acessar `alias._nome` de fora
gera um erro de compilação. Este é o único mecanismo de visibilidade; a linguagem
não possui as palavras-chave `pub` ou `priv`.

**Ponto de entrada `_main`.** O módulo de entrada deve definir `_main` como uma função de
um argumento, normalmente escrita como `_main = fn(args){ ... };`. A ausência de
`_main` no módulo de entrada é o erro de compilação `missing-main`, assim como um
`_main` que não seja uma função de um parâmetro. `_main` é privado pela
regra acima, portanto nenhum outro módulo pode chamá-lo: apenas o `main` do programa gerado o
chama. O runtime constrói uma lista `[string]` com os argumentos de linha de comando — o próprio nome do programa **não** faz parte da lista — e a passa
como o único argumento. Quando `_main` retorna um int, esse valor é o status de saída do executável; qualquer outro resultado é descartado e o status de saída
é 0. Como cada `_main` é privado ao seu módulo, uma única árvore de código fonte pode conter
vários arquivos de entrada (por exemplo, executores de teste) sem que seus pontos de entrada
colidam.

**`_init`.** Um módulo pode definir `_init = fn(...){ ... };`. É privado pela
regra acima e é chamado pelo sistema, não pelo código do programa:

* ele executa uma vez, com o argumento `[]`, e seu resultado é descartado;

* todas as constantes do módulo são inicializadas primeiro, em ordem de dependência: os
  imports de um módulo antes do próprio módulo, módulos independentes na ordem
  textual de seus imports, busca em profundidade; em seguida, cada `_init` executa, na
  mesma ordem; o `_init` do módulo de entrada, se houver, roda por último, logo
  antes de `_main`;

* como os nomes de nível superior são constantes, `_init` pode apenas realizar efeitos colaterais (por exemplo, saída de dados ou preparação de recursos de tempo de execução); não pode publicar estado para as outras funções do módulo.

Nada impede que o código no módulo chame `_init` pelo nome; fazer isso o executa novamente, o que é de responsabilidade do autor. O padrão recomendado para uma inicialização que também possa ser repetida sob demanda é manter o trabalho em uma função pública e fazer o `_init` apenas redirecionar para ela:

```c
init  = fn(x){ /* trabalho */ };
_init = fn(x){ init(x); };
```

**Comportamento removido.** O carregamento implícito do interpretador de `identificador.x` em uma chamada não resolvida não existe mais. O alvo de uma chamada deve ser uma vinculação visível ou um `alias.membro`. O `include` foi removido: ele não possuía um namespace próprio, e incluir um arquivo que é um módulo mesclaria o elemento incluído ao escopo do módulo. Código que usava `include` deve migrar para `import`.

**Pontos em aberto.** A opção de linha de comando `-I` e o relatório de caminho resolvido pertencem à Seção 8. Se o `import` também deve aceitar módulos compilados separadamente não está decidido. Tal módulo precisaria de um arquivo de interface com os tipos de seus membros públicos, e suas funções polimórficas ainda precisariam de seu código fonte para serem especializadas por tipo; até que isso seja decidido, `import` aceita apenas arquivos fonte.

### 4.2 Blocos

Um bloco `{ expr1; expr2; ... exprN; }` avalia todas as expressões sequencialmente e retorna o valor de `exprN`.

O valor de um bloco vazio é a lista vazia `[]`.

As instruções exigem `;` ao final. Portanto:

```c
{3;} // está correto
{3} // é um erro
```

E:

```c
{
  f = fn(x){ x + 9; };
}
```

Precisa de 2 ';': um após a expressão `(x+9)` e um após a definição da expressão `fn`.

Um programa é um módulo (veja 4.1), portanto seu nível superior contém apenas imports e definições `nome = inicializador;`. Onde quer que um bloco apareça — no corpo de uma função, incluindo `_main` — ele é uma expressão, e as regras acima se aplicam a ele: `{3;}` é um bloco válido, `{3}` é um erro, e uma definição `fn` dentro de um bloco precisa de um `;` após a expressão de seu corpo e um após a definição em si.

### 4.3 iif (Ternário)

`iif(cond, then_expr, else_expr)`:

* Avalia `cond`.

* Se `true`, avalia e retorna `then_expr`.

* Se `false`, avalia e retorna `else_expr`.

* As ramificações são avaliadas de forma preguiçosa (lazy evaluation).

* A condição deve ser avaliada como um bool. Nenhuma conversão implícita é realizada; uma condição não-bool lança `type-mismatch`.

* Ambas as ramificações devem ter o mesmo tipo, verificado em tempo de compilação. Uma chamada para `fail` se adapta à outra ramificação (veja 4.9).

### 4.4 Laço for

`for(init; cond; step) expr` — onde `expr` é geralmente um bloco:

* Executa `init`.

* Enquanto `cond` for verdadeiro:

  * Executa `expr` (o corpo do laço).

  * Executa `step`.

* Retorna o número de avaliações concluídas de seu corpo. Se a condição inicial for falsa, retorna `0`. Os valores produzidos pela inicialização, corpo e incremento são descartados. Uma falha em tempo de execução (veja 4.9) abandona o laço sem produzir um resultado. Um laço concluído com sucesso sempre retorna um int: zero ou uma contagem de iterações positiva.

* A condição deve ser avaliada como um bool; caso contrário, a avaliação lança `type-mismatch`. Nenhuma conversão implícita é realizada.

### 4.5 Funções

* Todas as funções têm a forma `y = f(x)`. Apenas um argumento para entrada e um argumento para saída. No entanto, `x` e/ou `y` podem ser listas, portanto: `[a, b] = f([c,d,e]);`

* Uma chamada sem parâmetros (`y = f()`) é equivalente a uma chamada com uma lista vazia (`y=f([])`).

* Note que `y = f(a,b)` é um erro de sintaxe. Funções XPR têm apenas um argumento. Use `y = f([a,b])` em seu lugar.

* `fn([a, b])` declara uma função de um argumento do tipo lista, desestruturado em `a` e `b` dentro do corpo (`f([1, 2])` vincula `a=1`, `b=2`).

  Um padrão de desestruturação consiste em identificadores ou padrões de lista aninhados recursivamente. Um padrão de lista requer uma lista ou uma tupla com o mesmo número de elementos naquele nível; um valor que não seja nenhum dos dois lança `type-mismatch`, e um número diferente de elementos é um `size-mismatch` (em tempo de compilação para uma tupla, quando a chamada executa para uma lista; veja 4.10). Um identificador recebe uma cópia do valor correspondente, inclusive quando esse valor é ele próprio uma lista. As mesmas regras de padrão se aplicam a atribuições de desestruturação.

  Por exemplo, com o argumento `[[2,3],[4,5]]`, `fn([a,b])` vincula listas inteiras a `a` e `b`, enquanto `fn([[a,b],[c,d]])` vincula `a=2`, `b=3`, `c=4`, `d=5`.

  Nomes de parâmetros repetidos seguem a mesma regra de atribuição da esquerda para a direita aplicável a alvos repetidos em uma atribuição por desestruturação. Assim, `f = fn([a,a]){ a; };` seguido de `f([10,11])` retorna `11`, sujeito ao tipo fixo da variável. Essas vinculações ocorrem no escopo local da função.

* Funções são valores: podem ser atribuídas a variáveis e passadas como argumentos como qualquer outro valor. Apenas preste atenção se `f` é a própria função ou uma chamada:

```c
f = fn(x){x + 9;};
use_func(f); // passa a função
use_value(f(1)); // passa o valor 10
```

* Não existem variáveis salvas:

```c
c = 9;
f9 = fn(x){x + c;}; // define uma função
f19 = fn(x){x + c;}; // define a mesma função
c = 19;   // não altera f9 ou f19
f19 = fn(x){x + c;}; // re-define uma função
print(f19(1));   // imprime 20
```

* Variáveis livres são resolvidas e congeladas no momento da definição. Qualquer nome livre sem vinculação visível, seja uma variável ou função, lança `undefined-identifier` na definição; o código de outros arquivos é alcançado apenas via `import` (veja 4.1):

```c
g = fn(x){ x + c; };   // erro: c ainda não está definido
```

* Um valor de função é um **closure**: seu nó de definição mais os valores de todas as variáveis livres, congelados **por cópia** no momento da definição (como no exemplo `f9`/`f19` acima). Reatribuir uma variável posteriormente nunca altera um closure que já a capturou.

* Uma variável capturada é **apenas um rvalue**. Dentro do corpo da função, seu valor é aquele congelado no momento da definição e nunca pode ser atribuído: uma atribuição (incluindo uma atribuição por desestruturação) cujo alvo seja um nome capturado de fora da função é um erro de compilação (`assign-to-captured`). Um nome é capturado quando está livre no corpo da função e possui uma vinculação visível no ponto onde a função é definida. Parâmetros e variáveis atribuídos pela primeira vez dentro do corpo da função são locais e podem ser atribuídos livremente, mesmo quando uma variável externa possui o mesmo nome (elas a sombreiam).

```c
{
    c = 10;
    f = fn(x){ c = c + x; c; };   // erro: `c` é capturado e não pode ser atribuído
    g = fn(x){ d = c + x; d; };   // válido: `d` é local, `c` é apenas lido
    h = fn(c){ c = c + 1; c; };   // válido: o parâmetro `c` sombreia o externo
}
```

  Para obter um valor mutável, a função calcula e retorna o novo valor, e o chamador o atribui. A linguagem não oferece meios para uma função alterar uma variável que reside fora dela.

* Nomes devem ser definidos antes do uso, seguindo a ordem das expressões no corpo da função. Os parâmetros são definidos ao entrar na definição da função, embora os valores de seus argumentos fiquem disponíveis apenas durante a invocação. Uma atribuição local pode depender desses valores; expressões subsequentes podem usar a variável local que ela define. Uma atribuição posterior não torna um uso anterior válido. A verificação dessa ordem não executa o corpo da função.

```c
f = fn(x){ print(y); y = x; };   // undefined-identifier na definição
```

```c
f = fn(x){ y = x; print(y); };   // válido: y é definido antes do uso
```

* Nomes de funções livres são resolvidos e seus valores de função são congelados por cópia quando a função envolvente é definida. A resolução nunca carrega arquivos; os imports de um módulo foram resolvidos antes da análise (veja 4.1). Chamadas escritas dentro do corpo da função não são executadas durante a definição; elas executam apenas quando a avaliação do corpo as alcança, usando os valores de função capturados e os valores de argumentos disponíveis naquele momento.

  O valor capturado inclui as próprias capturas congeladas da função, não apenas seu nó de definição AST ou o endereço de uma variável que contém a função. A AST registra a estrutura da chamada; cada closure em tempo de execução possui suas próprias capturas.

```c
{
    c = 10;
    helper = fn(x){ x + c; };
    f = fn(x){ helper(x); };
    c = 20;
    helper = fn(x){ x + 100; };
    f(1);
}
```

  Este programa retorna `11`: definir `f` captura o `helper` original, incluindo seu `c = 10` congelado, sem executar `helper(x)`. A chamada ocorre quando `f(1)` executa, após seu parâmetro ter recebido um valor.

* O próprio nome pendente da função pode ser capturado para recursão direta, mas não pode ser invocado antes que a função tenha sido totalmente inicializada.

* Em uma cadeia de atribuições de identificadores cuja expressão mais à direita seja uma definição de função, todos os nomes de destino são reconhecidos como vinculações pendentes antes que o valor da função seja criado. Referências a esses nomes dentro da função capturam a função sendo criada. As atribuições permanecem associativas à direita, e a função pendente não pode ser invocada antes que a inicialização seja concluída. Esses nomes de destino identificam a mesma função em construção; eles não introduzem recursão mútua entre definições de função distintas.

```c
{
    a = b = fn(x){ iif(x == 0, 0, a(x - 1)); };
    a(2);
}
```

  Este programa retorna `0`. O nome `a` é um auto-nome pendente quando a função é criada, mesmo que a atribuição mais interna tenha `b` como alvo.

* Dentro de `nome = fn(...){ ... }`, o nome sendo definido conta como uma variável livre que já está resolvida: o closure congela uma referência à função que está sendo criada (early binding), como uma declaração antecipada em C. O corpo nunca busca `nome` no escopo em tempo de chamada. Consequências:

```c
fat = fn(x){ iif(x<=1, 1, x * fat(x-1)); };   // corpo recursivo
g = fat;
fat = fn(x){ 999; };   // esta é uma nova função
g(5);                  // 120: a chamada recursiva foi vinculada à primeira
                       // função no momento da definição
fat(5);                // 999: esta chamada passa pela variável, que
                       // agora contém a nova função
```

  Se o corpo resolvesse `fat` através da variável em tempo de chamada (late binding), `g(5)` avaliaria `fat(4)` com a nova função e retornaria `999`.

* Uma função capturada como uma variável livre ordinária segue a mesma regra: `h = fn(x){ fat(x); };` continua chamando a função que existia quando `h` foi definida, mesmo se `fat` for reatribuída posteriormente.

* Parâmetros sombreiam globais: em `fn(c){ c + 1; }` o c no corpo é o parâmetro.

* Recursão mútua não é possível: todo nome livre é resolvido no momento da definição. Um `odd` que ainda não foi definido dentro de `even = fn(x){ ... odd(...) ...; };` lança `undefined-identifier`. Uma função de um módulo importado também não pode capturar um nome do arquivo importador: módulos são fechados, e apenas o próprio nome pendente de uma função pode ser capturado para recursão direta.

### 4.6. Escopo

Blocos definem escopo. Um programa roda em um único escopo global, e o programa em si é uma expressão. Variáveis só existem dentro do escopo de suas definições. Se uma função é atribuída a uma variável e essa variável sai de escopo, a definição também sai — a menos que outras variáveis contenham a mesma função.

Um escopo pode acessar variáveis definidas em seus escopos envolventes. Atribuição a uma variável existente atualiza a vinculação visível mais próxima; ela não cria uma nova variável local. A atribuição a um nome não definido cria uma variável no escopo atual. Variáveis criadas em um escopo interno não são visíveis fora dele.

Parâmetros de função criam vinculações locais, mesmo quando um escopo envolvente contém variáveis com os mesmos nomes.

```c
{
    x = 1;
    { x = 2; };
    x;
}
```

Este programa retorna `2`: o bloco interno atualiza o `x` existente, e o bloco externo retorna o valor de sua última expressão. Um bloco não vazio retorna o valor de sua última expressão; um bloco vazio retorna `[]` (veja 4.2). Esta regra de bloco não altera o resultado do `for`, que retorna o número de execuções do seu corpo (veja 4.4).

Variáveis e funções podem ser usadas apenas **após** sua definição. Dentro de funções, a distinção entre um nome definido cujo valor está pendente e um nome não definido segue a seção 4.5. Uma função não pode atribuir a um nome capturado (`assign-to-captured`); dentro da função ele é somente leitura.

```c
fat = fn(x){ iif(int(x) == 0 or int(x) == 1, 1, fat(int(int(x)-1))); };

c = fat(5);  // a variável fat já está definida
```

### 4.7 Atribuições

* Uma atribuição é uma expressão, portanto retorna o valor atribuído.

* Cadeias de atribuição são avaliadas da direita para a esquerda.

* Atribuições por desestruturação são realizadas da esquerda para a direita, recursivamente. Nomes de destino repetidos são permitidos. Cada atribuição atualiza a mesma vinculação, portanto a última atribuição bem-sucedida determina seu valor final, sujeito ao tipo fixo da variável. Assim, `[a,a] = [10,11]` deixa `a` igual a `11`.

* Uma variável possui um único tipo em seu escopo. O compilador o infere unificando os tipos de todas as atribuições a essa variável no texto do programa, independentemente da ordem de execução (veja a Seção 2). Int unificado com real resulta em real. Uma atribuição cujo tipo conflita com o tipo inferido gera um `type-mismatch` relatado em tempo de compilação, mesmo em código que nunca executa. Um nome vinculado a literais de função segue as regras da seção 4.10.

Exemplo (com `a` e `b` indefinidos anteriormente):

```c
a = [a, b] = [1, 2]; // erro: `a` seria um int (elemento) e um [int]
a = [a, b] = [[1, 2], 3];  // erro: `a` seria [int] (primeiro elemento)
                           // e a tupla [[int], int] (o valor inteiro)
[a, b] = [[1, 2], 3];      // ok: a é [int], b é int
c = [a, b];                // ok: c é a tupla [[int], int]
```

Com tipos estáticos, as duas primeiras atribuições são rejeitadas em tempo de compilação com `type-mismatch`; elas eram aceitas pelo interpretador anterior apenas quando os tipos em tempo de execução coincidiam.

### 4.8 Valores e cópias

Tudo é um valor; não há referências visíveis ao usuário. Atribuição e passagem de parâmetros preservam a semântica de valor: reatribuir uma variável não altera o valor mantido por outra variável.

Valores de lista e string são imutáveis. A atribuição pode substituir o valor inteiro de uma variável, sujeito ao seu tipo fixo, mas expressões indexadas são somente leitura e não podem ser alvos de atribuição. Esta restrição também se aplica a listas aninhadas e a elementos dentro de padrões de desestruturação.

Operações que alteram o conteúdo de uma lista retornam uma nova lista. Elas não modificam um valor de lista existente. Elementos e sublistas imutáveis podem compartilhar armazenamento interno quando suas durações de vida forem garantidas.

```c
{
    a = [1, 2];
    b = a;
    b = [9, b[1]];
    println(a);
    println(b);
}
```

Aqui `a` permanece `[1, 2]` e `b` torna-se `[9, 2]`. Atribuir `a` a `b` pode copiar apenas um descritor e compartilhar os elementos imutáveis. A atribuição posterior substitui `b` por um novo valor de lista sem alterar `a`.

Uma função pode da mesma forma substituir todo o valor de um parâmetro sem alterar a variável do chamador. Capturas congeladas mantêm seus valores do momento da definição e não podem ser atribuídas (veja 4.5).

A cópia física profunda (deep copy) não é necessária: como os valores são imutáveis, uma implementação pode compartilhar armazenamento entre valores, e o compartilhamento não é observável pelo programa. A implementação deve manter o armazenamento compartilhado ativo enquanto qualquer valor o utilizar (a implementação usa contagem de referências, por exemplo `Rc`/`Arc` no código Rust gerado; veja [diretrizes-runtime.md](diretrizes-runtime.md)).

A indexação permanece disponível para leitura, incluindo índices sucessivos em listas aninhadas. Ler um elemento de lista retorna seu valor. Ler um índice de string retorna uma string de um codepoint. Por exemplo, `[10, 20][1]` retorna `20` e `[[1, 2], [3, 4]][1][0]` retorna `3`.

Uma vírgula dentro dos colchetes é um atalho para indexação sucessiva: `v[a, b, c]` é `v[a][b][c]`, avaliado da esquerda para a direita. Assim, `[[1, 2], [3, 4]][1, 0]` também retorna `3`. É apenas uma forma mais curta de escrever índices sucessivos; não seleciona vários elementos. Selecionar vários elementos usa uma lista de índices como índice único, com seus próprios colchetes: `v[[2, 0, 2]]` (veja `at` na Seção 5), que é uma expressão diferente de `v[2, 0, 2]`. A regra para tuplas permanece inalterada: cada índice em uma tupla deve ser uma constante.

Um índice deve ser um int. Um índice de outro tipo, incluindo um real, é um `type-mismatch` em tempo de compilação. Um índice negativo, ou não menor que o comprimento do valor indexado, resulta na falha em tempo de execução `index-out-of-range`. Listas nunca crescem implicitamente; use `cat` para construir uma lista concatenada.

Indexar uma **tupla** requer um índice constante: um literal int conhecido em tempo de compilação, porque cada posição pode ter um tipo diferente. Um índice calculated em uma tupla é um `type-mismatch` em tempo de compilação, e um índice constante fora do limite da tupla é um `index-out-of-range` em tempo de compilação. Indexar uma lista aceita qualquer expressão int; o intervalo é verificado em tempo de execução.

### 4.9 Falhas: try e fail

Algumas operações podem falhar apenas quando o programa executa, porque o resultado depende de valores: divisão por zero, um real que não cabe em um int, um índice fora da lista ou string, um tamanho negativo, listas de tamanhos diferentes, texto que não é um número, fim da entrada, o limite de profundidade de chamadas e um `fail` explícito. Problemas que dependem apenas de nomes e tipos (identificadores não definidos, incompatibilidades de tipo, tamanhos de tuplas, índices constantes de tuplas, atribuições a nomes capturados, erros de módulo) são relatados em tempo de compilação e não podem ser capturados.

Uma **falha não capturada** relata um diagnóstico no stderr com a posição do código fonte, seguido pela dica do catálogo quando houver, descarrega o buffer de saída (flush) e encerra o programa com status diferente de zero. Efeitos já realizados não são desfeitos.

**`try(expr)`** é uma forma especial, não uma função: a falha ocorre enquanto `expr` está sendo avaliada, portanto `expr` não deve ser avaliada antes que o `try` esteja no controle. Se `expr` tem o tipo `T`, então `try(expr)` tem o tipo tupla `[bool, T]`:

* quando `expr` é concluída, o resultado é `[true, valor]`;

* quando uma falha ocorre em qualquer lugar durante a avaliação de `expr`, inclusive dentro das funções que ela chama, a avaliação de `expr` é abandonada e o resultado é `[false, neutro]`, onde `neutro` é o valor neutro de `T`. A avaliação então continua após o `try`.

```c
[ok, q] = try(a / b);
[ok, n] = try(int(input("idade: ")));
```

Regras:

* O primeiro elemento é `true` exatamente quando nenhuma falha ocorreu. O motivo de uma falha capturada é descartado e nenhum diagnóstico é impresso para ela.

* Valores criados por uma avaliação abandonada são descartados. Efeitos colaterais já realizados, como saída de dados, permanecem.

* Chamadas a `try` podem ser aninhadas. Uma falha vai para o `try` mais interno que estiver avaliando no momento em que a falha ocorrer, o que pode estar dentro de uma função chamada por ele. Uma falha é capturada pelo `try` apenas uma vez; para retransmiti-la, teste `ok` e chame `fail`.

* Toda falha em tempo de execução pode ser capturada, exceto falta de memória e erros internos do runtime, que encerram o programa.

* O valor neutro de um tipo é: `0` para int, `0.0` para real, `""` para string, `false` para bool, `[]` para qualquer lista, e para uma tupla é a tupla dos valores neutros de suas posições. Um tipo de função não possui valor neutro, nem uma tupla que contenha uma posição de função. Aplicar `try` a uma expressão de tal tipo é um erro de compilação.

**`fail(mensagem)`** é um builtin que sempre falha. Seu argumento deve ser uma string. Seu tipo de resultado é livre: assume o tipo exigido pelo contexto, como o tipo de elemento de `[]`, de modo que `iif(n >= 0, n, fail("negativo"))` tenha o tipo de `n`. Quando não capturado, o diagnóstico exibe a `mensagem` e a posição da chamada. Quando capturado por um `try`, a mensagem é descartada.

```c
age = fn(s){
    [ok, n] = try(int(s));
    iif(ok and n >= 0, n, fail("idade invalida"));
};
[ok, a] = try(age(input("idade: ")));
```

### 4.10 Tipos de função, anotações e especialização

Toda função possui um parâmetro e um resultado, portanto seu tipo é `A -> R`, onde `A` e `R` são cada um um tipo escalar, uma lista ou uma tupla. O tipo do resultado é o tipo da última expressão do corpo; não há anotação de resultado.

**Padrões de parâmetros.**

* `fn(x)` aceita qualquer tipo; `fn(xs:[])` aceita qualquer lista, de qualquer comprimento.

* `fn([a, b])` é um padrão de exatamente dois elementos. Aceita uma tupla de dois elementos, verificada em tempo de compilação, ou uma lista, cujo comprimento é verificado quando a função é chamada: uma lista de outro comprimento gera a falha `size-mismatch`. Uma tupla com outro número de posições gera um `size-mismatch` em tempo de compilação, e um valor que não seja lista ou tupla gera um `type-mismatch`.

* Padrões podem ser aninhados: `fn([[a, b], [c, d]])`.

* `f()` é `f([])`; seu argumento é uma lista vazia.

* Uma variável de lista pode ser passada onde um padrão tem o mesmo tamanho: `l = [1, 2]; f(l);` com `f = fn([a, b]){ ... }` é válido.

**Anotações.** Uma folha do padrão pode conter `:tipo`, como em `fn([a:int, b:string])`. Uma anotação é opcional. Ela restringe os tipos que o parâmetro aceita, e um argumento de outro tipo gera um `type-mismatch` relatado na chamada. Os nomes de tipos são:

| Tipo | Significado | 
| ----- | ----- | 
| `bool`, `int`, `real`, `string` | aquele tipo | 
| `fn` | uma função (sua assinatura vem da inferência) | 
| `[]` | qualquer lista | 
| `[T]` | uma lista cujos elementos têm o tipo `T` | 
| `[T1, T2, ...]` | uma tupla com esses tipos de posições; se todos forem iguais, é a lista `[T]` | 

A anotação `[T1, ..., Tn]` com tipos iguais não fixa um comprimento: comprimentos são verificados apenas por padrões.

**Funções genéricas e especialização.** Um tipo é *concreto* quando é `bool`, `int`, `real`, `string`, uma lista de um tipo concreto ou uma tupla de tipos concretos. Uma função é *genérica* em todo parâmetro cujo tipo não seja concreto (sem anotação, ou `[]` ou `fn`). O compilador gera uma instância da função para cada combinação distinta de tipos de argumentos concretos com os quais ela é usada, com os tipos de seu corpo e resultado resolvidos para essa combinação. Uma função cujos parâmetros estão todos anotados com tipos concretos possui uma única instância. Não há sobrecarga declarada pelo usuário.

* Uma função usada como um valor é instanciada com os tipos que seu contexto exige.

* O tipo de resultado de uma função recursiva é determinado pelas ramificações que não chamam a própria função. Se nenhuma ramificação o determinar, a função é rejeitada em tempo de compilação com `cannot-infer`.

* Uma função genérica exportada por um módulo é instanciada no programa que a utiliza, e é por isso que `import` aceita arquivos fonte (veja 4.1).

* Os nomes das funções no código gerado são descritos em [diretrizes-runtime.md](diretrizes-runtime.md).

**Funções como valores.** A genericidade existe apenas em tempo de compilação: um programa em execução nunca mantém uma função genérica. Um nome vinculado com `nome = fn(...){...}` é uma *vinculação genérica*, e o compilador rastreia a qual definição cada uso do nome se refere:

* Uma chamada `nome(arg)` cujo uso é alcançado por exatamente uma definição instancia essa definição com o tipo de `arg`. É uma chamada direta: nenhum valor de função existe em tempo de execução, e a chamada pode ser expandida em linha (inline).

* O uso do nome **como um valor** (passado como argumento, retornado, armazenado em uma lista, uma ramificação de `iif`) instancia a definição com o tipo `A -> R` exigido pelo contexto, e o valor é um closure daquele tipo concreto. Se o contexto não determinar `A`, o programa é rejeitado com `cannot-infer`; uma anotação no parâmetro resolve o problema.

* Reatribuir o nome a outro literal de função cria uma nova vinculação a partir daquele ponto do texto. Closures anteriores mantêm a definição que capturaram. Em `g = fat; fat = fn(x){ 999; };` (veja 4.5), `g` é a primeira função e `fat` é a segunda.

* Quando mais de uma definição pode alcançar um uso (o nome é atribuído dentro de uma ramificação de `iif` ou no corpo de um laço, ou o valor flui através de uma variável reatribuída condicionalmente), o nome mantém um valor de função em tempo de execução e possui **um** tipo concreto `A -> R`. Todas as definições que alcançam o uso devem instanciar para esse mesmo tipo, caso contrário o programa é rejeitado com `type-mismatch`. Chamadas ocorrem através do valor da função.

Builtins também possuem assinaturas (Seção 5). Alguns builtins aceitam vários tipos de argumentos; o compilador seleciona o caso a partir do tipo do argumento.

## 5. Funções Integradas (Built-in)

Cada builtin recebe um argumento e retorna um resultado (veja 4.10). `α` e `β` representam qualquer tipo, e `[α]` representa uma lista de qualquer tipo de elemento. Um builtin que lista vários casos é selecionado pelo tipo de seu argumento em tempo de compilação: o compilador gera o caso para aquele tipo concreto, de modo que este polimorfismo não tem custo em tempo de execução. Os renderizadores `print`, `println`, `string` e `format` aceitam qualquer tipo, exceto um valor de função, seja diretamente ou aninhado em uma lista ou tupla: um programa que renderizaria uma função é rejeitado em tempo de compilação. Strings não são listas, mas `len`, `at` e `slice` possuem seus próprios casos para strings.

| Builtin | Argumento | Resultado | 
| ----- | ----- | ----- | 
| `print`, `println` | `α` | `int` | 
| `len` | `[α]` ou `string` | `int` | 
| `input` | `string` | `string` | 
| `fail` | `string` | `β` (qualquer tipo) | 
| `string` | `α` | `string` | 
| `format` | `[α, [int]]` | `string` | 
| `int` | `int`, `real` ou `string` | `int` | 
| `real` | `int`, `real` ou `string` | `real` | 
| `floor`, `ceil`, `round` | `real` | `real` | 
| `frac` | `real` | `real` | 
| `any`, `all` | `[bool]` | `bool` | 
| `cat` | `[[α], [α]]` | `[α]` | 
| `at` | `[[α], int]`, `[string, int]` | `α`, `string` | 
| `at` | `[[α], [int]]`, `[string, [int]]` | `[α]`, `string` | 
| `fill` | `[int, α]` | `[α]` | 
| `range` | `int`, `[int, int]` ou `[int, int, int]` | `[int]` | 
| `set` | `[[α], int, α]` | `[α]` | 
| `slice` | `[[α], int, int]`, `[string, int, int]` | `[α]`, `string` | 
| `apply` | `[[α], α -> β]` | `[β]` | 

* `print(x)`: Imprime o argumento no stdout. Retorna o número de **codepoints** impressos. Strings são impressas sem aspas envolventes, inclusive dentro de listas e tuplas. Bools são impressos como `true` ou `false`. Ints são impressos como dígitos decimais. Reals usam o formato padrão (veja 5.1). Listas e tuplas são impressas recursivamente com colchetes e uma vírgula seguida por um espaço entre os elementos, como em `[10.00, 20.00, 30.00]` para reals; uma lista vazia é impressa como `[]`. `print` não adiciona nova linha: escreva `\n` em uma string quando necessário. Caracteres de nova linha explícitos contam para o número de codepoints retornados.

* `println(x)`: o mesmo que `print(x)` seguido por uma nova linha. A contagem retornada inclui a nova linha. Uma string é impressa sem aspas e sem colchetes (`println("hi")` escreve `hi` e uma nova linha), enquanto uma lista ou tupla ainda imprime sua estrutura com colchetes. Para imprimir vários itens em uma única linha, construa uma única string: `println("x = " + string(x))`. Não escreva `print([x, "\n"])`: isso é uma lista ou tupla, e seus colchetes e separadores serão impressos.

* `len(lista/string)`: Retorna o comprimento como um int (para strings, em codepoints). O uso de `len` com qualquer outro tipo gera um `type-mismatch` em tempo de compilação.

* `input(prompt)`: Lê uma linha do stdin e a retorna como uma string. No fim da entrada, falha com `end-of-input`. Converter o texto para outro tipo é explícito, através dos builtins de conversão.

* `fail(mensagem)`: encerra a avaliação com uma falha contendo a `mensagem`; veja 4.9.

* `string(val)`: converte qualquer valor para o texto que o `print` exibiria.

* `format([v, [decimais, alg_significativos]])`: como o `string`, mas com formato numérico explícito (veja 5.1).

* `int(val)`: a partir de um real, trunca em direção a zero. Falha com `overflow` quando o real for NaN, infinito ou estiver fora do intervalo de int. A partir de uma string, analisa um inteiro decimal e falha com `invalid-number` em caso contrário. A partir de um int, retorna o próprio valor. `real(val)`: a partir de um int, converte-o. A partir de uma string, analisa um real, incluindo os textos `NaN`, `INF` e `-INF`, e falha com `invalid-number` caso contrário. A partir de um real, retorna o próprio valor. Combine-os com `try` quando o texto vier de fora do programa. Não há conversão para `number` ou `bool`: o tipo resultante de uma conversão não deve depender do valor, e a verificação de valores booleanos é escrita explicitamente com comparações (`n != 0`, `s != ""`, `l != []`, `s == "true"`).

* `floor(x)`, `ceil(x)`, `round(x)`, `frac(x)`: arredondamento IEEE-754 padrão (o mesmo
  comportamento de `math.h` em C ou de `f64::floor`/`ceil`/`round` em Rust). Recebem um real e retornam um real. `floor` e `ceil` arredondam para baixo e para cima, respectivamente; `round` arredonda o valor meio para longe do zero; `frac` retorna a parte fracionária com sinal (`frac(x) == x - real(int(x))`). NaN e infinitos passam inalterados (`frac` deles é NaN). Para obter um int, converta explicitamente: `int(floor(x))`; é nessa conversão que um valor fora do intervalo falha.

| x | int | floor | ceil | round | frac | 
| ----- | ----- | ----- | ----- | ----- | ----- | 
| 3.99 | 3 | 3.0 | 4.0 | 4.0 | 0.99 | 
| \-3.7 | \-3 | \-4.0 | \-3.0 | \-4.0 | \-0.7 | 
| 5.5 | 5 | 5.0 | 6.0 | 6.0 | 0.5 | 

* `int`, `real`, `floor`, `ceil`, `round` e `frac` aplicam-se elemento por elemento em listas e, posição por posição, em tuplas, produzindo os tipos de resultado descritos acima: `round([1.2, 3.7])` resulta em `[1.0, 4.0]`.

* `any(lista)`: verdadeiro se pelo menos um elemento for verdadeiro; `any([])` é `false`.

* `all(lista)`: verdadeiro apenas se todos os elementos forem verdadeiros; `all([])` é `true`.

  Tanto `any` quanto `all` aceitam apenas listas de bools. Nenhuma conversão implícita ou nivelamento recursivo é realizado. Outros tipos de argumentos geram um `type-mismatch` em tempo de compilação.

* `cat([a, b])`: recebe um par de duas listas com o mesmo tipo de elemento e as concatena. Por exemplo, `cat([[1,2],[3]])` retorna `[1,2,3]`.

* `at([v, i])`: recebe um par composto por uma lista ou string e um índice int. Retorna o elemento `i` (baseado em 0). Para strings, retorna uma string de um codepoint. `v[i]` é um atalho sintático para `at([v, i])`; aplicam-se as regras de índice da seção 4.8. Quando o índice for uma lista de ints, `at` funciona como uma seleção: `at([v, [2, 0, 2]])` é `[v[2], v[0], v[2]]`, com o mesmo tipo de elemento de `v` (para uma string, a string formada pelos codepoints selecionados). Cada índice deve ser válido, caso contrário gera `index-out-of-range`; uma lista de índices vazia resulta em um resultado vazio.

* `fill([n, x])`: uma lista de `n` cópias de `x`, do tipo `[α]` onde `α` é o tipo de `x`. As cópias compartilham o mesmo valor imutável: `fill([r, fill([c, 0.0])])` contém uma linha, referenciada `r` vezes. `n == 0` resulta em `[]`; um `n` negativo falha com `invalid-size`. Não há argumento de tipo: `fill([3, 0])` é `[int]` e `fill([3, 0.0])` é `[real]`.

* `range(n)`, `range([a, b])`, `range([a, b, passo])`: uma lista de ints. A forma com `passo` existe apenas no `range`. `range(n)` é `range([0, n])`, e `range([a, b])` é `range([a, b, 1])`. Com um `passo` positivo, a lista é `a, a+passo, ...` enquanto for menor que `b`; com um `passo` negativo, é `a, a+passo, ...` enquanto for maior que `b`. Por exemplo, `range([2, 5])` é `[2, 3, 4]` e `range([5, 0, -2])` é `[5, 3, 1]`. Um intervalo vazio, ou `passo == 0`, resulta em `[]`; nunca resulta em falha.

* `set([v, i, x])`: uma nova lista igual a `v` com o elemento `i` substituído por `x`. A lista `v` não é alterada: `set` possui semântica de valor, e se o resultado reutiliza o armazenamento de `v` é uma questão de implementação. O elemento deve ser unificável com o tipo de elemento de `v`. Um índice fora de `v` falha com `index-out-of-range`.

* `slice([v, a, b])`: os elementos de `a` até `b-1` de uma lista, ou os codepoints de uma string, mantendo o mesmo tipo de `v`. Os limites são ajustados (clamped) para `0..len(v)`, e `a >= b` resulta em um resultado vazio; `slice` nunca falha. O resultado é sempre uma lista contígua: `slice` não possui passo e não há formas com salto (strided). Selecionar com um passo é feito via `at([v, range([a, b, passo])])`, que constrói uma nova lista contígua. Outros layouts (visões multidimensionais ou com saltos) pertencem à biblioteca `tensor`, não às listas básicas.

* `apply([xs, f])`: aplica a função `f` a cada elemento da lista `xs`, do primeiro ao último, e retorna a lista com os resultados. Se `xs` for um `[α]` e `f` for do tipo `α -> β`, o resultado será um `[β]`; `f` é um valor de função instanciado com o tipo de elemento de `xs` (veja 4.10). `xs` não é alterado: `apply` possui semântica de valor, e se o resultado reutiliza o armazenamento de `xs` é uma questão de implementação, assim como no `set`.

  * `f` recebe cada elemento como seu único argumento, e padrões podem desestruturá-lo: `apply([pairs, fn([a, b]){ a + b; }])`. Pode ser uma função do usuário ou um builtin: `apply([["a", "bb"], len])` resulta em `[1, 2]`.

  * `apply` é raso (shallow): ele atua nos elementos do nível superior. Para uma lista de listas, aplique em cada linha: `apply([m, fn(row){ apply([row, g]); }])`.

  * Uma lista é homogênea, portanto cada resultado possui o mesmo tipo `β`. Uma função cujo tipo de resultado dependa do elemento é rejeitada em tempo de compilação.

  * Uma falha dentro de `f` é a falha de todo o `apply`, sem resultado parcial (veja 4.9); saídas produzidas por chamadas anteriores permanecem. Como as capturas são somente leitura, a ordem das chamadas é observável apenas por meio de efeitos colaterais como `print` e `input`.

  * **Lista vazia é absorvente.** `apply([[], f])` resulta em `[]` para **qualquer** `f`, e `f` não
    é instanciado nesse caso: sem elementos, não há tipo de elemento que determine a instância. A
    lista vazia é o elemento neutro da aplicação, como o zero é da soma — a operação não tem o que
    transformar. Isso vale inclusive quando `f` é genérica e seu corpo seria inválido para algum
    tipo (`apply([[], fn(x){ x * 2; }])` é válido): o corpo nunca receberia um elemento.

  * **`f` é instanciado no uso** (Seção 4.10), com o tipo de elemento de `xs`. Um `f` definido como
    `f = fn(x){ … }` é uma vinculação genérica — nada é instanciado na atribuição; a instância é
    criada nesta chamada. Assim, `apply([["a"], fn(x){ x + 1; }])` é `type-mismatch` (a instância
    string de `x + 1` é inválida), enquanto `apply([["a", "b"], fn(x){ x + "1"; }])` resulta em
    `["a1", "b1"]`. O resultado tem o tipo de resultado **daquela** instância.

  * No momento, `apply` aceita apenas listas. Ele poderá posteriormente receber suporte para strings, assim como `len`, `at` e `slice`, sem alterar o caso de listas; até lá, textos são manipulados por funções de string separadas.

* Um builtin que constrói uma lista a partir de listas retorna uma lista vazia quando não há nada para construir (`cat` de listas vazias, `slice` de uma lista vazia, `range` de um intervalo vazio, `at` com uma lista de índices vazia, `apply` de uma lista vazia; o literal `[]` absorve qualquer função, veja abaixo). Apenas operações que devem produzir ou posicionar um único elemento podem falhar: `at` com um índice int, `set`, e `fill` com tamanho negativo.

* Funções matemáticas como `sqrt`, `exp`, `sin` e `log`, assim como operações matriciais e de listas além das descritas acima, não são builtins: pertencem a bibliotecas (um módulo `math` da biblioteca padrão está planejado) que seguem a mesma regra elemento a elemento.

* Builtins são símbolos globais predefinidos e nunca são ocultados por arquivos acessados via `import`: a biblioteca padrão vem primeiro no caminho de busca (veja 4.1). Uma definição do usuário vincula um valor de função anônima a uma variável.

* Builtins mantêm valores de função e podem ser passados como argumentos como qualquer função (`use_func(print)` é válido).

### 5.1 Formatação de números

Não há uma configuração global de formatação. `print`, `println` e `string` convertem **reals** para texto com um formato padrão fixo, `decimais = 2` e `alg_significativos = 5`; ints são sempre impressos como dígitos decimais. Para escolher outro formato, converta explicitamente:

```c
format([x, [4, 15]])     // x com 4 casas decimais e 15 dígitos significativos: uma string
println(format([x, [4, 15]]));
```

`format([v, [decimais, alg_significativos]])` recebe qualquer valor `v` e uma lista de dois ints, retornando uma string. Ele formata cada real contido em `v` (inclusive dentro de listas e tuplas) com esse formato, e todo o restante exatamente como `string` faz. Um segundo argumento que não seja uma lista de dois ints gera um `size-mismatch` ou `type-mismatch`; um valor de `decimais` negativo ou `alg_significativos` menor que 1 falha com `invalid-size`.

O formato `[decimais, alg_significativos]` funciona da seguinte maneira:

* `NaN`, `INF` e `-INF` são impressos literalmente, ignorando o formato.

* Se $10^{-\text{decimais}} \le |x| < 10^{\text{alg\_significativos}}$: notação fixa com `decimais` dígitos após o ponto — o real `20.0` é impresso como `20.00`.

* Caso contrário: notação científica com `alg_significativos` dígitos significativos — no formato padrão `[2, 5]`, `0.001` é impresso como `1.0000e-3` e `1234567890123456789.0` como `1.2346e+18`. Com formato explícito `[2, 15]`, este último é impresso como `1.23456789012346e+18`.

* O arredondamento segue a convenção round-half-to-even (arredonda o meio para o par mais
  próximo), o mesmo comportamento do `printf` de C.

* `real(string(x)) == x` não é garantido; `real(format([x, [17, 17]])) == x` é garantido para todo real finito.

Outros tipos e campos de formatação adicionais ainda não estão especificados.

### 5.2 Semântica dos operadores

| operandos | `+` | `-` `*` `/` `%` | `< <= > >=` | `== !=` | `not`/`and`/`or` | 
| ----- | ----- | ----- | ----- | ----- | ----- | 
| int ⊗ int | soma | aritmética; `/` é divisão inteira | bool | bool | — | 
| real ⊗ real | soma | aritmética | bool | bool | — | 
| int ⊗ real (qualquer ordem) | soma, int promovido a real | aritmética, int promovido a real | bool, após promoção | bool, após promoção | — | 
| string ⊗ string | concatenação | type-mismatch | bool, lexicográfico por codepoint ("A" < "a") | bool | — | 
| lista ⊗ lista, mesmo tamanho | elemento a elemento | elemento a elemento | elemento a elemento | elemento a elemento | — | 
| lista ⊗ lista, tamanhos diferentes | size-mismatch (tempo de execução) | size-mismatch (tempo de execução) | size-mismatch (tempo de execução) | size-mismatch (tempo de execução) | — | 
| tupla ⊗ tupla, mesmo tamanho | posição a posição | posição a posição | posição a posição | posição a posição | — | 
| tupla ⊗ tupla, tamanhos diferentes | size-mismatch (tempo de compilação) | size-mismatch (tempo de compilação) | size-mismatch (tempo de compilação) | size-mismatch (tempo de compilação) | — | 
| tupla ⊗ lista | type-mismatch | type-mismatch | type-mismatch | type-mismatch | — | 
| escalar ⊗ lista | type-mismatch (sem broadcasting) | type-mismatch | type-mismatch | type-mismatch | — | 
| bool ⊗ bool | type-mismatch | type-mismatch | type-mismatch | bool | bool; `and`/`or` com curto-circuito | 
| operandos do tipo função | type-mismatch | type-mismatch | type-mismatch | type-mismatch | — | 

Como o comprimento de uma lista pertence ao seu valor e não ao seu tipo, uma diferença de comprimento entre duas listas é uma falha em tempo de execução. Quando o compilador puder provar a partir de constantes que a falha ocorrerá (por exemplo, `[1, 2] + [1]`), ele poderá emitir um aviso, mas não rejeitará o programa: se um programa é aceito ou não, isso não deve depender de quão longe a análise do compilador alcança.

Listas usam uma única regra, aplicada recursivamente em todos os níveis: a operação é aplicada elemento por elemento e, quando ambos os elementos de um par forem listas, a mesma regra é aplicada novamente. Tuplas seguem a mesma regra posição por posição, e cada posição é verificada contra seu próprio tipo em tempo de compilação: por exemplo, `[1, "a"] + [2, "b"]` resulta na tupla `[3, "ab"]`, enquanto `[1, "a"] - [2, "b"]` gera um `type-mismatch` em tempo de compilação. O tamanho de uma tupla é conhecido quando o programa é compilado, portanto uma diferença de tamanho entre tuplas é detectada nesse momento.

* `+`: folhas numéricas (int ou real) somam; folhas string concatenam; pares de listas aplicam a regra recursivamente (`[[1],[2]] + [[3],[4]]` resulta em `[[1]+[3], [2]+[4]]`, que resulta em `[[4],[6]]`).

* `-`, `*`, `/`, `%`: apenas folhas numéricas; pares de listas aplicam a regra recursivamente.

* `/` entre dois ints é uma divisão inteira, truncando em direção a zero (como em C). Para dividir ints como reals, converta explicitamente: `real(a) / real(b)`. Se qualquer um dos operandos for um real, o resultado será um real.

* `%` é definido apenas para dois ints e possui o sinal do dividendo, de modo que `(a / b) * b + a % b == a`. Um operando real gera um `type-mismatch` em tempo de compilação.

* Divisão ou `%` por zero falha com `div-by-zero`, tanto para ints quanto para reals.

* As operações `+`, `-`, `*` e o `-` unário para Int sofrem overflow/underflow em aritmética de complemento de dois: o resultado é o valor exato módulo $2^{64}$. Este é um comportamento definido, nunca uma falha. Casos especiais seguem a mesma regra: `-INT_MIN` é `INT_MIN`, `INT_MIN / -1` é `INT_MIN` e `INT_MIN % -1` é `0`. Esse estouro de limite é o comportamento necessário para hashes e codificações em nível de bit; o código que precisa detectar overflow deve verificar os operandos explicitamente. Apenas a conversão de um real que não cabe em um int (NaN, um infinito ou um valor fora do intervalo) falha, gerando `overflow`.

* Comparações: folhas numéricas/string produzem um bool, coletado em uma lista (`[a,b] == [c,d]` resulta em `[a==c, b==d]`); reduza com `any`/`all`.

* Concatenação de listas nunca é feita com `+`; use `cat([a, b])`.

* Os operadores unários numéricos `+` e `-` aplicam-se recursivamente aos elementos da lista. Cada elemento que não for uma lista deve ser numérico (int ou real); caso contrário, o programa é rejeitado com `type-mismatch`.
  Por exemplo, `-[10,-11,12,-13]` produz `[-10,11,-12,13]`.

* NaN segue o padrão IEEE-754: qualquer comparação envolvendo NaN resulta em falso.

## 6. Constantes Matemáticas

Constantes matemáticas predefinidas são vinculações somente leitura. Atribuir a uma constante é um erro. A comparação de igualdade lê seu valor normalmente. Assim, `pi = 3.14` é inválido, enquanto `pi == 3.14` é válido e avaliado como `false`. Atribuir a uma constante predefinida é um erro de compilação (`assign-to-constant`).
Não existe a palavra-chave `const`. As constantes de um programa são as definições no nível superior de um módulo (veja 4.1), que são definidas uma vez, avaliadas pelo compilador e somente leitura para todas as funções. Dentro de um programa ou função, um nome é uma variável comum; um compilador ainda pode tratar um nome atribuído apenas uma vez como uma constante, como uma otimização que não altera o significado do programa.

* `true`, `false` : os dois valores booleanos; são constantes, não palavras-chave

* `NaN` : Not a Number / Não é um Número (real)

* `INF` : Infinito (real) - pode ser usado com os sinais unários "+" ou "-"

* `pi` ou `PI` : o número pi (real)

* `e` ou `E` : o número de Euler (real). `exp` não é uma constante: está reservado para a função exponencial da biblioteca `math`

## 7. Erros

O `xprc` compila todo o programa antes que qualquer execução ocorra, portanto os erros existem em duas fases: erros em **tempo de compilação**, relatados pelo `xprc` (sintaxe, nomes, tipos, módulos), e **falhas em tempo de execução**, relatadas pelo programa compilado (veja 4.9). Erros de sintaxe são detectados mesmo dentro de corpos de funções que nunca são chamadas, assim como erros de tipo e de nome.

A compilação é interrompida no primeiro erro. Um diagnóstico é emitido uma vez no stderr. Erros relacionados ao código fonte incluem o nome da fonte, linha e coluna, com colunas contadas em codepoints; o código gerado embute essas posições diretamente (como literais nas chamadas ao runtime de diagnóstico) para que as falhas em tempo de execução apontem para o arquivo fonte `.x`, e não para o código Rust gerado. Erros sem uma localização no código fonte relatam o arquivo, operação ou argumento relevante. Uma compilação com falha ou uma falha em tempo de execução não capturada retorna um status de processo diferente de zero, independentemente da enumeração interna de diagnósticos. Efeitos colaterais já realizados não são desfeitos automaticamente.

**Fonte única para definições de diagnóstico**

O catálogo de diagnósticos é a fonte autoritativa para códigos, identificadores públicos,
severidades, modelos de mensagem e dicas. Ele vive em um único módulo Rust compartilhado entre o
compilador e o runtime do programa gerado (veja [diretrizes-runtime.md](diretrizes-runtime.md)),
garantindo que ambos relatem exatamente os mesmos IDs, mensagens e dicas a partir de uma única
definição, sem duplicação manual.
Esta especificação não mantém uma tabela normativa de diagnósticos separada. Qualquer referência de diagnóstico gerada deve derivar suas entradas do catálogo em vez de duplicá-las manualmente.

As seções semânticas desta especificação determinam quais operações devem falhar e quando. Nomes de erro mencionados aqui descrevem as condições semânticas; o catálogo determina o identificador e a apresentação real do diagnóstico. Se um nome em outro lugar da documentação diferir do catálogo, o catálogo prevalece para a definição do diagnóstico. Uma condição de erro exigida ainda não representada no catálogo permanece como uma tarefa de implementação; sua ausência não torna a operação válida.

No programa compilado, há um único ponto de falha no runtime: ele registra o diagnóstico; se não houver um `try` ativo, ele relata o diagnóstico — ID, mensagem e, quando presente, a dica, todos vindos do catálogo —, descarrega os buffers de saída e encerra o processo com um código de saída diferente de zero. Se houver um `try` ativo, ele faz com que cada chamada envolvente retorne, liberando o que possuía, até que o `try` seja alcançado. Esse encerramento nunca é um `panic!`/abort abrupto do processo: o diagnóstico sempre é relatado e os buffers são descarregados antes de o processo terminar.

**Condições de erro obrigatórias**

* A análise sintática rejeita sintaxe inválida, alvos de atribuição inválidos e um código fonte completo que não contenha nenhuma expressão. Alvos de atribuição devem ser identificadores ou padrões de lista recursivos contendo apenas folhas identificadoras. Expressões indexadas nunca são alvos de atribuição válidos.

* A compilação rejeita tipos incompatíveis, identificadores não definidos, chamadas para valores que não são funções, tamanhos de tuplas divergentes, índices de tuplas que não são constantes ou estão fora do intervalo, renderização de um valor de função (5), atribuições a constantes somente leitura e a nomes capturados (4.5), erros de módulo (4.1), tipos que não podem ser inferidos (4.10) e `try` sobre um tipo sem valor neutro (4.9).

* Um programa em execução falha ao realizar divisão por zero, quando um real não cabe em um int, com índices inválidos de listas e strings, com tamanho negativo, listas de tamanhos diferentes, texto que não é um número, fim de entrada, estouro do limite de profundidade de chamadas e ao encontrar um `fail` explícito.

* A profundidade de chamada de função é limitada a 10.000; ultrapassar esse limite é uma falha em tempo de execução. Este limite de profundidade de chamada é separado do aninhamento de `try`. A linguagem não possui um limite de passos de avaliação: um programa que não termina deve ser interrompido externamente, por exemplo, pelo sistema operacional ou por um limite de tempo (timeout).

## 8. Interface de linha de comando

O compilador é invocado como `xprc [opções] <nome_do_arquivo.x>`. Ele traduz o programa para Rust e o compila (usando `rustc`, diretamente ou através de `cargo`); ele não executa o programa. Não há modo interativo, nenhuma expressão fornecida via linha de comando e nenhum código fonte lido do stdin. As formas suportadas são:

| Invocação | Comportamento | 
| ----- | ----- | 
| `xprc <nome_do_arquivo.x>` | Traduz para Rust e compila. Gera o executável com o nome do arquivo sem `.x`, no diretório atual. | 
| `xprc <nome_do_arquivo.x> -o <arquivo>` | O mesmo que o anterior, com o executável nomeado como `<arquivo>`. | 
| `xprc --emit-rust <nome_do_arquivo.x>` | Escreve apenas o Rust gerado, no arquivo `-o` se fornecido, ou no stdout. O `rustc`/`cargo` não é executado. | 
| `xprc --lex-dump <nome_do_arquivo.x>` | Exibe os tokens do arquivo para depuração (dump). | 
| `xprc --parse-dump <nome_do_arquivo.x>` | Exibe a árvore sintática gerada do arquivo para depuração. | 
| `xprc --type-dump <nome_do_arquivo.x>` | Exibe os tipos inferidos para depuração. | 
| `xprc -h` ou `xprc --help` | Exibe a ajuda de uso. | 
| `xprc --version` | Exibe a versão. | 

Opções:

| Opção | Significado | 
| ----- | ----- | 
| `-o <arquivo>` | Nome da saída (executável ou arquivo Rust com `--emit-rust`). | 
| `-I <dir>` | Adiciona um diretório ao caminho de busca de `import` (veja 4.1). Pode ser repetido; os diretórios são buscados em ordem. | 
| `--rustc <comando>` | Compilador Rust a ser utilizado. Padrão: `rustc`. | 
| `-O <nível>` | Nível de otimização de `0` a `3`, traduzido para o `-C opt-level` correspondente do `rustc`. Padrão: `2`. | 
| `-v` | Relata o caminho resolvido de cada arquivo carregado por `import`, e o comando executado no compilador Rust. | 

Regras:

* Exatamente um arquivo fonte é aceito. Executar `xprc` sem arquivo fonte e sem opção de modo exibe a ajuda de uso e falha; ele nunca aguarda por entrada de dados.

* O diretório do arquivo fonte é o "diretório do arquivo" no caminho de busca da seção 4.1.

* A ajuda exibe primeiramente a versão do compilador.

* O executável não precisa do `xprc` para rodar: o runtime é vinculado a ele.
  Seu `main` (no código Rust gerado) constrói a lista de argumentos e chama o `_main` do módulo de entrada
  (veja 4.1). O arquivo de entrada deve definir `_main`; caso contrário, a compilação
  falha com `missing-main`. A saída de dados nunca é implícita: ela deve ser solicitada
  explicitamente através de funções de saída.

* `input` lê a entrada padrão do executável. O fim da entrada falha com `end-of-input` (veja 5 e 4.9).

* Diagnósticos vão para o stderr (veja a Seção 7). Uma compilação com falha retorna um status diferente de zero e não gera executável.
