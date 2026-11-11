// Module exported to main.typ via #import "chapters/methods.typ": architecture-table

#let architecture-table() = {
  figure(
    table(
      columns: (auto, 1.2fr, 2fr),
      inset: (x: 8pt, y: 6pt),
      align: (left, left, left),
      fill: (x, y) => if y == 0 { rgb("#1e293b") } else if calc.odd(y) { rgb("#f8fafc") } else { white },
      stroke: 0.5pt + rgb("#cbd5e1"),
      table.header(
        [#text(fill: white, weight: "bold")[Subsystem]],
        [#text(fill: white, weight: "bold")[Module]],
        [#text(fill: white, weight: "bold")[Desktop Responsibility]],
      ),
      [*Desktop Menu & UI*], [`src/app.rs`], [Native OS menu bar on macOS, top menu bar on Linux/Win, Settings window],
      [*Disk Project I/O*], [`src/project.rs`], [Direct `std::fs` folder scanning, file tree, and disk read/write],
      [*Compiler World*], [`src/world.rs`], [Implements `typst::World` over the local project directory on disk],
      [*Page Rasterizer*], [`typst-render`], [Renders `typst::layout::Page` frames to RGBA GPU textures],
      [*PDF Exporter*], [`typst-pdf` + `rfd`], [Exports PDF directly to disk via native file dialogs],
    ),
    caption: [Native desktop architecture across Linux, macOS, and Windows.],
  )
}
