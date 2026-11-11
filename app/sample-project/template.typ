// Reusable document template & components for Typst Studio

#let project(
  title: "",
  subtitle: none,
  authors: (),
  date: none,
  version: none,
  body,
) = {
  set document(title: title, author: authors.map(a => a.name))
  set page(
    paper: "a4",
    margin: (x: 1.8cm, top: 2.0cm, bottom: 2.0cm),
    header: context {
      if counter(page).get().first() > 1 [
        #set text(size: 8.5pt, fill: rgb("#64748b"))
        #grid(
          columns: (1fr, 1fr),
          align(left)[#emph(title)],
          align(right)[Typst Studio #if version != none [(#version)]],
        )
        #v(-4pt)
        #line(length: 100%, stroke: 0.4pt + rgb("#cbd5e1"))
      ]
    },
    footer: context [
      #set text(size: 8.5pt, fill: rgb("#64748b"))
      #line(length: 100%, stroke: 0.4pt + rgb("#e2e8f0"))
      #v(2pt)
      #grid(
        columns: (1fr, 1fr),
        align(left)[Compiled by Typst 0.12 Engine],
        align(right)[Page #counter(page).display("1 of 1", both: true)],
      )
    ],
  )

  set text(font: "Libertinus Serif", size: 10.5pt, lang: "en")
  set par(justify: true, leading: 0.62em)
  set heading(numbering: "1.1")

  show heading.where(level: 1): it => block(above: 1.3em, below: 0.7em)[
    #set text(size: 13.5pt, weight: "bold", fill: rgb("#0f172a"))
    #it
    #v(-4pt)
    #line(length: 100%, stroke: 1pt + rgb("#3b82f6"))
  ]

  show heading.where(level: 2): it => block(above: 1.1em, below: 0.55em)[
    #set text(size: 11.5pt, weight: "bold", fill: rgb("#1e3a8a"))
    #it
  ]

  show raw.where(block: false): box.with(
    fill: rgb("#f1f5f9"),
    inset: (x: 3pt, y: 0pt),
    outset: (y: 3pt),
    radius: 2pt,
  )

  block(
    width: 100%,
    fill: rgb("#0f172a"),
    inset: (x: 16pt, y: 14pt),
    radius: 6pt,
  )[
    #set text(fill: white)
    #grid(
      columns: (1fr, auto),
      align(left)[
        #text(size: 8.5pt, weight: "bold", fill: rgb("#38bdf8"))[NATIVE DESKTOP TYPST WORKSPACE]
        #v(3pt)
        #text(size: 18pt, weight: "bold")[#title]
        #if subtitle != none [
          #v(3pt)
          #text(size: 10.5pt, fill: rgb("#cbd5e1"))[#subtitle]
        ]
      ],
      if version != none {
        align(right + top)[
          #box(
            fill: rgb("#1e293b"),
            stroke: 0.8pt + rgb("#38bdf8"),
            inset: (x: 7pt, y: 4pt),
            radius: 4pt,
          )[#text(size: 8.5pt, weight: "bold", fill: rgb("#38bdf8"))[#version]]
        ]
      }
    )
    #v(8pt)
    #grid(
      columns: (1fr, auto),
      align(left)[
        #set text(size: 9pt, fill: rgb("#e2e8f0"))
        #authors.map(a => [*#a.name* (#a.affiliation)]).join("  •  ")
      ],
      if date != none [
        #align(right)[#text(size: 9pt, fill: rgb("#94a3b8"))[#date]]
      ]
    )
  ]

  v(8pt)
  body
}

#let callout(title: "Note", tone: "info", body) = {
  let (bg, border, fg) = if tone == "warn" {
    (rgb("#fffbeb"), rgb("#f59e0b"), rgb("#92400e"))
  } else {
    (rgb("#eff6ff"), rgb("#3b82f6"), rgb("#1e40af"))
  }
  block(
    width: 100%,
    fill: bg,
    stroke: (left: 3.5pt + border, rest: 0.5pt + border.lighten(50%)),
    inset: (x: 12pt, y: 9pt),
    radius: (right: 4pt),
  )[
    #text(weight: "bold", size: 9.5pt, fill: fg)[#title]
    #v(3pt)
    #set text(size: 9.5pt, fill: rgb("#1e293b"))
    #body
  ]
}

#let metric-grid(..cards) = {
  let items = cards.pos()
  grid(
    columns: (1fr,) * items.len(),
    gutter: 10pt,
    ..items.map(card => {
      let (label, value, sub) = card
      block(
        width: 100%,
        fill: rgb("#f8fafc"),
        stroke: 0.7pt + rgb("#cbd5e1"),
        inset: (x: 10pt, y: 8pt),
        radius: 5pt,
      )[
        #text(size: 8pt, weight: "bold", fill: rgb("#64748b"))[#upper(label)]
        #v(2pt)
        #text(size: 13pt, weight: "bold", fill: rgb("#0f172a"))[#value]
        #v(2pt)
        #text(size: 8pt, fill: rgb("#475569"))[#sub]
      ]
    })
  )
}
