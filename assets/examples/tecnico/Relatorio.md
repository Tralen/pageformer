---
title: Relatório Técnico
author: Pageformer
language: pt-BR
date: 2026-03-10
subject: Demonstração das funcionalidades do template
keywords:
  - relatório
  - demonstração
---

# Relatório Técnico

Este relatório demonstra, em um documento curto, o cabeçalho e o rodapé correntes, a quebra de página explícita, o estilo de tabela, as formatações em linha e os blocos de citação e de código.

## Escopo

O exemplo usa um template próprio nesta pasta, que substitui o template compartilhado da pasta pai. Sem timbre, as duas margens ficam livres para o conteúdo corrente, e o nível 1 abre cada seção em uma página nova.

### Formatações em linha

O texto aceita *ênfase*, **negrito**, ~~riscado~~ e `código em linha`, além de [links](https://example.com) e notas de rodapé.[^nota]

[^nota]: A nota de rodapé é recolhida ao final do documento.

## Listas

- Item simples;
- Item com **negrito** e `código`;
  - Subitem com marcador de segundo nível;
    - Sub-subitem com marcador de terceiro nível;
- Item com [site de exemplo](https://example.com).

1. Primeiro passo;
   1. Passo interno com numeração alfabética;
   2. Outro passo interno;
2. Segundo passo;
3. Terceiro passo.

- [x] Tarefa concluída;
- [ ] Tarefa pendente.

A tabela abaixo usa a borda, o preenchimento do cabeçalho e a cor definidos no template.

| Etapa | Prazo | Responsável |
|---|---|---|
| Levantamento | 5 dias | Análise |
| Execução | 10 dias | Produção |
| Revisão | 3 dias | Qualidade |

O cabeçalho mostra o autor à esquerda, o título ao centro e a numeração à direita. O rodapé repete o aviso à esquerda e o nome do programa à direita em todas as páginas.

## Citação e alerta

> Uma citação em bloco, com `código` e *ênfase*, para mostrar a barra vertical e o fundo declarados no template.

Um alerta usa a mesma sintaxe de citação, com um tipo entre colchetes:

> [!NOTE]
> Um alerta é uma citação com tipo, e mantém o rótulo entre colchetes.

## Código

O bloco abaixo usa a face monoespaçada, o fundo e os filetes declarados no template.

```rust
fn main() {
    println!("olá");
}
```

Uma régua fecha esta seção.

---

## Resultados

A paginação e a numeração são calculadas pelo motor de composição a partir do documento inteiro, de modo que o total de páginas no cabeçalho corresponde ao número final.

O mesmo modelo gera os quatro formatos. O PDF usa o motor Typst como biblioteca, o ODT e o DOCX são montados por escritores internos e o TXT é uma vista do modelo para copiar como texto simples. Como o template é um só, a face, as cores, os espaçamentos e os blocos acompanham todos eles.

A tabela abaixo resume o que cada formato preserva do template.

| Formato | Layout | Cores | Blocos |
|---|---|---|---|
| PDF | paginação própria | sim | sim |
| ODT | paginação do Writer | sim | sim |
| DOCX | paginação do Word | sim | sim |
| TXT | sem layout | não | texto |

O bloco seguinte força uma quebra de página explícita antes do anexo. Sem o marcador, o anexo continuaria na mesma página.

{pagebreak}

# Anexo

O anexo começa em uma página nova por causa do parágrafo que contém apenas o marcador de quebra de página. Qualquer outra ocorrência, como {pagebreak} nesta frase, permanece texto comum.

# Apêndice

Este apêndice começa em uma página nova por causa de `page_break_before` no nível 1 do template, sem nenhum marcador no texto. A quebra é fraca, então não cria uma página vazia quando o título já está no topo.
