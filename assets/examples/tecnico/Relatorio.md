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

Este relatório demonstra, em um documento curto, o cabeçalho e o rodapé correntes, a quebra de página explícita, o estilo de tabela e os metadados do documento.

## Escopo

O exemplo usa um template próprio nesta pasta, que substitui o template compartilhado da pasta pai. Sem timbre, as duas margens ficam livres para o conteúdo corrente.

| Etapa | Prazo | Responsável |
|---|---|---|
| Levantamento | 5 dias | Análise |
| Execução | 10 dias | Produção |
| Revisão | 3 dias | Qualidade |

O cabeçalho mostra o autor à esquerda, o título ao centro e a numeração à direita. O rodapé repete o aviso à esquerda e o nome do programa à direita em todas as páginas.

## Resultados

A paginação e a numeração são calculadas pelo motor de composição a partir do documento inteiro, de modo que o total de páginas no cabeçalho corresponde ao número final.

O bloco seguinte força uma quebra de página explícita antes do anexo. Sem o marcador, o anexo continuaria na mesma página.

{pagebreak}

# Anexo

O anexo começa em uma página nova por causa do parágrafo que contém apenas o marcador de quebra de página. Qualquer outra ocorrência, como {pagebreak} nesta frase, permanece texto comum.

# Apêndice

Este apêndice começa em uma página nova por causa de `page_break_before` no nível 1 do template, sem nenhum marcador no texto. A quebra é fraca, então não cria uma página vazia quando o título já está no topo.
