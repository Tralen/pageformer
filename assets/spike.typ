#set document(
  title: "Contrato de Prestação de Serviços",
  author: "Pageformer",
  date: datetime(year: 2026, month: 1, day: 1),
)

#set page(
  paper: "a4",
  margin: (top: 32mm, bottom: 24mm, left: 30mm, right: 20mm),
  background: place(
    top,
    rect(width: 100%, height: 6mm, fill: rgb("#1f3a5f")),
  ),
  header: context [
    #set text(size: 8pt, fill: rgb("#555555"))
    #grid(
      columns: (1fr, auto),
      [Escritório Exemplo],
      [Contrato de Prestação de Serviços],
    )
    #v(-4pt)
    #line(length: 100%, stroke: 0.5pt + rgb("#999999"))
  ],
  footer: context [
    #set text(size: 8pt, fill: rgb("#555555"))
    #align(center)[
      Página #counter(page).display("1 de 1", both: true)
    ]
  ],
)

#set text(
  font: ("Liberation Serif", "DejaVu Serif"),
  size: 12pt,
  lang: "pt",
)
#set par(justify: true, leading: 0.65em)
#set heading(numbering: "1.")

#show heading.where(level: 1): it => pagebreak(weak: true) + it

#align(center)[
  #text(size: 16pt, weight: "bold")[CONTRATO DE PRESTAÇÃO DE SERVIÇOS]
  #v(2pt)
  #text(size: 10pt, style: "italic")[Instrumento particular de prestação de serviços]
]

#v(12pt)

#heading[Das partes]

Pelo presente instrumento particular, de um lado, *CONTRATANTE*, pessoa jurídica de direito privado, inscrita no Cadastro Nacional da Pessoa Jurídica sob o número indicado em sua qualificação completa, com sede no endereço constante de seus atos constitutivos, neste ato representada na forma da lei, e, de outro lado, *CONTRATADA*, pessoa jurídica de direito privado, inscrita no Cadastro Nacional da Pessoa Jurídica sob o número indicado em sua qualificação completa, com sede no endereço constante de seus atos constitutivos, neste ato representada na forma da lei, resolvem celebrar o presente contrato, que se regerá pelas cláusulas e condições a seguir.

As partes declaram, para todos os efeitos, que têm plena ciência do objeto deste instrumento e que a assinatura ao final representa a manifestação livre e informada de sua vontade.

#heading[Do objeto]

O presente contrato tem por objeto a prestação de serviços técnicos especializados, conforme o escopo descrito no Anexo I, que integra este instrumento para todos os efeitos, independentemente de transcrição.

A CONTRATADA compromete-se a executar os serviços com diligência, observando as melhores práticas do setor, os prazos ajustados e as orientações recebidas da CONTRATANTE, desde que compatíveis com o escopo contratado.

#heading[Das obrigações da contratada]

Sem prejuízo das demais obrigações previstas neste instrumento, incumbe à CONTRATADA:

#list(
  [executar os serviços com qualidade técnica e diligência;],
  [manter sigilo sobre as informações a que tiver acesso;],
  [fornecer relatórios periódicos de andamento;],
  [comunicar de imediato qualquer fato que possa comprometer o prazo ou o resultado.]
)

A CONTRATADA responderá pelos materiais e pelas informações que produzir no âmbito deste contrato, bem como pela correção de vícios verificados no prazo de garantia ajustado entre as partes.

#heading[Das obrigações da contratante]

Incumbe à CONTRATANTE fornecer as informações, os documentos e os acessos necessários à execução dos serviços, bem como efetuar os pagamentos nas condições e nos prazos previstos neste instrumento.

A CONTRATANTE deverá indicar um preposto para acompanhar a execução dos serviços e centralizar as comunicações com a CONTRATADA, evitando instruções contraditórias ou desencontradas.

#heading[Do preço e das condições de pagamento]

Pela execução dos serviços, a CONTRATANTE pagará à CONTRATADA o valor total indicado na tabela abaixo, em parcelas vinculadas às etapas de entrega.

#table(
  columns: (1fr, 2fr, 1fr),
  inset: 6pt,
  align: (left, left, right),
  stroke: 0.5pt + rgb("#bbbbbb"),
  table.header(
    [Parcela],
    [Descrição],
    [Valor],
  ),
  [1], [Mobilização e serviços iniciais], [R\$ 5.000,00],
  [2], [Serviços intermediários], [R\$ 5.000,00],
  [3], [Entrega final e encerramento], [R\$ 5.000,00],
)

O pagamento de cada parcela será devido em até cinco dias úteis após o aceite da etapa correspondente, mediante apresentação de documento fiscal hábil.

O atraso no pagamento sujeitará a CONTRATANTE a multa de dois por cento sobre o valor em atraso, acrescida de juros de um por cento ao mês, calculados pro rata die.

#heading[Do prazo]

O prazo de execução dos serviços é de noventa dias, contados da assinatura deste instrumento, podendo ser prorrogado por acordo escrito entre as partes.

A CONTRATADA comunicará à CONTRATANTE, com antecedência razoável, qualquer fato que possa afetar o cronograma, apresentando as medidas necessárias para mitigar o impacto.

#heading[Da confidencialidade]

#quote(block: true, attribution: "Cláusula sexta")[
  A confidencialidade das informações trocadas entre as partes permanece obrigatória mesmo após o término deste contrato.
]

As partes obrigam-se a não divulgar, a terceiros, as informações sigilosas a que tiverem acesso em razão deste contrato, salvo mediante autorização escrita da outra parte ou por determinação legal.

#heading[Das disposições gerais]

Qualquer alteração deste contrato somente terá eficácia se formalizada por escrito e assinada por representantes de ambas as partes.

A tolerância de uma parte quanto ao descumprimento de obrigação pela outra não constitui renúncia ao direito de exigir o cumprimento futuro, nem novação da obrigação original.

Os casos omissos serão resolvidos de boa-fé pelas partes, à luz da legislação aplicável e dos princípios que regem a contratação.

#v(24pt)

#align(center)[
  #text(size: 10pt)[Local e data:]
  #v(6pt)
  #line(length: 60%, stroke: 0.5pt)
]

#v(24pt)

#grid(
  columns: (1fr, 1fr),
  gutter: 20pt,
  [
    #align(center)[
      #line(length: 80%, stroke: 0.5pt)
      #text(size: 9pt)[CONTRATANTE]
    ]
  ],
  [
    #align(center)[
      #line(length: 80%, stroke: 0.5pt)
      #text(size: 9pt)[CONTRATADA]
    ]
  ],
)
