//! Unit tests for the pure Typst IntelliSense engine (completions, hover, and hints).

use std::path::{Path, PathBuf};

use super::*;

#[test]
fn reference_completions_on_at_and_prefix_include_typ_labels_and_bib_entries() {
    let main_src = r#"= Introduction <sec-intro>
This work evaluates collaborative editing.

#figure(
  image("arch.png"),
  caption: [System architecture],
) <fig-arch>

#table(columns: 2)[A][B] <tab-results>

See @
"#;
    let bib_src = r#"@article{knuth84,
  author = {Donald E. Knuth},
  title = {Literate Programming},
  year = {1984},
}

@book{tapl,
  author = {Benjamin C. Pierce},
  title = {Types and Programming Languages},
  year = {2002},
}
"#;
    let sources = [
        (Path::new("main.typ"), main_src),
        (Path::new("refs.bib"), bib_src),
    ];

    // Empty prefix right after '@' returns all .typ labels and .bib citation keys.
    let cursor = main_src.chars().count() - 1; // right after '@'
    let ctx = compute_completions(main_src, cursor, &sources, &[])
        .expect("expected reference completions right after '@'");
    assert_eq!(ctx.prefix, "");
    assert_eq!(ctx.replace_start_char, cursor);
    assert_eq!(ctx.replace_end_char, cursor);

    let labels: Vec<&str> = ctx.items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"sec-intro"));
    assert!(labels.contains(&"fig-arch"));
    assert!(labels.contains(&"tab-results"));
    assert!(labels.contains(&"knuth84"));
    assert!(labels.contains(&"tapl"));

    let knuth = ctx.items.iter().find(|i| i.label == "knuth84").unwrap();
    assert_eq!(knuth.kind, CompletionKind::Reference);
    assert_eq!(knuth.detail, "Article");
    assert!(knuth.documentation.contains("Literate Programming"));
    assert!(knuth.documentation.contains("Donald E. Knuth"));
    assert!(knuth.documentation.contains("1984"));
    assert_eq!(knuth.source_hint.as_deref(), Some("refs.bib:1"));

    let fig = ctx.items.iter().find(|i| i.label == "fig-arch").unwrap();
    assert_eq!(fig.detail, "Figure");
    assert!(fig.documentation.contains("System architecture"));

    // Filtered prefix '@tab-' returns only matching table label and replaces 'tab-'.
    let filtered_src = format!("{main_src}And @tab-");
    let end_cursor = filtered_src.chars().count();
    let tab_ctx = compute_completions(&filtered_src, end_cursor, &sources, &[])
        .expect("expected filtered reference completions for @tab-");
    assert_eq!(tab_ctx.prefix, "tab-");
    assert_eq!(tab_ctx.replace_end_char - tab_ctx.replace_start_char, 4);
    assert_eq!(tab_ctx.items.len(), 1);
    assert_eq!(tab_ctx.items[0].label, "tab-results");
    assert_eq!(tab_ctx.items[0].detail, "Table");
}

#[test]
fn code_and_macro_completions_include_user_let_docs_typst_builtins_and_keywords() {
    let macros_src = r#"/// Renders a highlighted callout box.
/// Supports custom titles.
#let callout(title, body) = {
  block(title + ": " + body)
}

// Constant accent color
#let accent_color = blue
"#;
    let active = "#fig";
    let sources = [(Path::new("macros.typ"), macros_src)];

    let ctx = compute_completions(active, active.chars().count(), &sources, &[])
        .expect("expected completions for #fig");
    assert_eq!(ctx.prefix, "fig");
    assert_eq!(ctx.replace_start_char, 1);
    assert_eq!(ctx.replace_end_char, 4);
    let fig = ctx
        .items
        .iter()
        .find(|i| i.label == "figure")
        .expect("figure builtin missing");
    assert_eq!(fig.kind, CompletionKind::Function);
    assert!(!fig.detail.is_empty());
    assert!(!fig.documentation.is_empty());

    // '#table' and '#text' have built-in Typst docs and signatures.
    let table_ctx = compute_completions("#tab", 4, &sources, &[]).unwrap();
    assert!(table_ctx
        .items
        .iter()
        .any(|i| i.label == "table" && !i.documentation.is_empty()));

    let text_ctx = compute_completions("#tex", 4, &sources, &[]).unwrap();
    assert!(text_ctx
        .items
        .iter()
        .any(|i| i.label == "text" && !i.documentation.is_empty()));

    // '#' alone surfaces user macros, variables, builtins, and keywords.
    let all_ctx = compute_completions("#", 1, &sources, &[]).unwrap();
    let callout = all_ctx
        .items
        .iter()
        .find(|i| i.label == "callout")
        .expect("user macro callout missing");
    assert_eq!(callout.kind, CompletionKind::Macro);
    assert_eq!(callout.detail, "(title, body)");
    assert!(callout
        .documentation
        .contains("Renders a highlighted callout box."));
    assert!(callout.documentation.contains("Supports custom titles."));
    assert_eq!(callout.source_hint.as_deref(), Some("macros.typ:3"));

    let accent = all_ctx
        .items
        .iter()
        .find(|i| i.label == "accent_color")
        .expect("user variable accent_color missing");
    assert_eq!(accent.kind, CompletionKind::Variable);
    assert_eq!(accent.documentation, "Constant accent color");

    for kw in ["let", "set", "show", "import", "include", "context"] {
        assert!(
            all_ctx
                .items
                .iter()
                .any(|i| i.label == kw && i.kind == CompletionKind::Keyword),
            "missing keyword {kw}"
        );
    }
}

#[test]
fn math_mode_and_code_block_expressions_complete_without_hash() {
    let math_src = "$alp";
    let math_ctx =
        compute_completions(math_src, 4, &[], &[]).expect("expected math completions inside $...$");
    assert_eq!(math_ctx.prefix, "alp");
    assert_eq!(math_ctx.replace_start_char, 1);
    assert!(math_ctx.items.iter().any(|i| i.label == "alpha"));

    let block_src = "#{ let x = 1; tex";
    let block_ctx = compute_completions(block_src, block_src.chars().count(), &[], &[])
        .expect("expected code completions inside #{...}");
    assert_eq!(block_ctx.prefix, "tex");
    assert!(block_ctx.items.iter().any(|i| i.label == "text"));

    let param_src = "#figure(cap";
    let param_ctx = compute_completions(param_src, param_src.chars().count(), &[], &[])
        .expect("expected parameter completions inside #figure(...)");
    assert!(param_ctx
        .items
        .iter()
        .any(|i| i.kind == CompletionKind::Parameter
            && i.label == "caption"
            && i.insert_text == "caption: "));
}

#[test]
fn file_path_completions_work_for_import_include_image_and_bibliography() {
    let files = vec![
        PathBuf::from("main.typ"),
        PathBuf::from("chapters/intro.typ"),
        PathBuf::from(r"assets\diagram.png"),
        PathBuf::from("refs.bib"),
    ];

    let imp = "#import \"ch";
    let imp_ctx = compute_completions(imp, imp.chars().count(), &[], &files)
        .expect("expected file path completions for #import");
    assert_eq!(imp_ctx.prefix, "ch");
    assert_eq!(imp_ctx.replace_start_char, 9);
    assert_eq!(imp_ctx.items[0].label, "chapters/intro.typ");
    assert_eq!(imp_ctx.items[0].kind, CompletionKind::FilePath);

    let img = "#image(\"";
    let img_ctx = compute_completions(img, img.chars().count(), &[], &files)
        .expect("expected image path completions");
    assert_eq!(img_ctx.items[0].label, "assets/diagram.png");

    let bib = "#bibliography(\"ref";
    let bib_ctx = compute_completions(bib, bib.chars().count(), &[], &files)
        .expect("expected bibliography path completions");
    assert_eq!(bib_ctx.items[0].label, "refs.bib");
}

#[test]
fn hover_covers_diagnostics_refs_macros_and_typst_builtins() {
    let helper_src = "/// Formats a badge.\n#let badge(label) = [#label]\n";
    let active =
        "= Overview <sec-ov>\nSee @sec-ov and @missing.\n#badge(\"ok\")\n#text(size: 10pt)[hi]\n";
    let sources = [(Path::new("helpers.typ"), helper_src)];

    // Compiler diagnostic takes highest priority when covering hover_char.
    let diags = [(0usize, 10usize, "Custom compiler error".to_string(), true)];
    let diag_hover =
        compute_hover_info(active, 3, &sources, &diags).expect("expected diagnostic hover");
    assert!(diag_hover.is_diagnostic);
    assert_eq!(diag_hover.documentation, "Custom compiler error");

    // Resolved @sec-ov hover shows kind, file:line, and snippet.
    let ov_ref_idx = active.find("@sec-ov").unwrap() + 2;
    let ov_hover = compute_hover_info(active, ov_ref_idx, &sources, &[])
        .expect("expected resolved @ref hover");
    assert!(!ov_hover.is_diagnostic);
    assert_eq!(ov_hover.title, "@sec-ov");
    assert!(ov_hover.detail.contains("Heading"));
    assert!(ov_hover.documentation.contains("Overview"));

    // Unresolved @missing hover returns is_diagnostic: true.
    let miss_idx = active.find("@missing").unwrap() + 1;
    let miss_hover = compute_hover_info(active, miss_idx, &sources, &[])
        .expect("expected unresolved @ref hover");
    assert!(miss_hover.is_diagnostic);
    assert!(miss_hover.title.contains("missing"));

    // User #let macro hover shows signature and doc comment.
    let badge_idx = active.find("badge").unwrap() + 1;
    let badge_hover =
        compute_hover_info(active, badge_idx, &sources, &[]).expect("expected user macro hover");
    assert_eq!(badge_hover.title, "badge");
    assert!(badge_hover.detail.contains("(label)"));
    assert_eq!(badge_hover.documentation, "Formats a badge.");

    // Built-in #text hover shows Typst docs.
    let text_idx = active.find("text(").unwrap() + 1;
    let text_hover =
        compute_hover_info(active, text_idx, &sources, &[]).expect("expected builtin text hover");
    assert_eq!(text_hover.title, "text");
    assert!(!text_hover.documentation.is_empty());
}

#[test]
fn unresolved_ref_hints_and_unicode_char_offsets_are_exact() {
    let src = "= 🚀 Überschrift <sec-uni>\nText mit café @sec-uni und @fehlt!\n#fig";
    let hints = compute_typst_hints(src, &[], &[(0, 4, "Notice".to_string(), false)]);
    assert_eq!(hints.len(), 2);
    assert_eq!(hints[0].message, "Notice");
    assert!(!hints[0].is_error);

    let miss_hint = &hints[1];
    assert!(miss_hint.is_error);
    assert!(miss_hint.message.contains("@fehlt"));
    let chars: Vec<char> = src.chars().collect();
    let sliced: String = chars[miss_hint.start_char..miss_hint.end_char]
        .iter()
        .collect();
    assert_eq!(sliced, "@fehlt");

    // Multi-byte emoji + umlaut before '#fig': char offset must match chars().count().
    let cursor = src.chars().count();
    let comp = compute_completions(src, cursor, &[], &[])
        .expect("expected completions after multi-byte Unicode");
    assert_eq!(comp.prefix, "fig");
    assert_eq!(comp.replace_start_char, cursor - 3);
    assert_eq!(comp.replace_end_char, cursor);
    let fig_slice: String = chars[comp.replace_start_char..comp.replace_end_char]
        .iter()
        .collect();
    assert_eq!(fig_slice, "fig");
}

#[test]
fn ignores_comments_raw_blocks_escaped_hash_and_emails() {
    assert!(compute_completions("// @sec", 7, &[], &[]).is_none());
    assert!(compute_completions("`#fig`", 5, &[], &[]).is_none());
    assert!(compute_completions("email@domain", 12, &[], &[]).is_none());
    assert!(compute_completions("\\#fig", 5, &[], &[]).is_none());
    assert!(compute_completions("Plain prose word", 16, &[], &[]).is_none());
}

#[test]
fn dotted_stdlib_completions_and_hover_and_arrow_closures_work() {
    let calc_src = "#calc.si";
    let calc_ctx = compute_completions(calc_src, calc_src.chars().count(), &[], &[])
        .expect("expected dotted completions for #calc.si");
    assert_eq!(calc_ctx.prefix, "si");
    assert!(calc_ctx.items.iter().any(|i| i.label == "sin"));

    let hover_sin =
        compute_hover_info("#calc.sin(1)", 7, &[], &[]).expect("expected hover for calc.sin");
    assert_eq!(hover_sin.title, "calc.sin");
    assert!(!hover_sin.documentation.is_empty());

    let arrow_math = "$arrow.r$";
    let hover_arrow =
        compute_hover_info(arrow_math, 7, &[], &[]).expect("expected hover for arrow.r");
    assert_eq!(hover_arrow.title, "arrow.r");

    let closure_let = "// Computes sum\n#let add = (a, b) => a + b\n#add";
    let closure_ctx = compute_completions(closure_let, closure_let.chars().count(), &[], &[])
        .expect("expected completion for arrow closure #let");
    let add_item = closure_ctx.items.iter().find(|i| i.label == "add").unwrap();
    assert_eq!(add_item.kind, CompletionKind::Macro);
    assert_eq!(add_item.detail, "(a, b)");
    assert_eq!(add_item.documentation, "Computes sum");
}

#[test]
fn heading_where_hover_and_doc_sanitization_produce_clean_docs() {
    let show_src = "#show heading.where(level: 1): set text(blue)\n";

    // Hovering over `heading` in `heading.where(...)` resolves `heading`.
    let heading_idx = show_src.find("heading").unwrap() + 2;
    let heading_hover =
        compute_hover_info(show_src, heading_idx, &[], &[]).expect("expected hover for heading");
    assert_eq!(heading_hover.title, "heading");
    assert!(!heading_hover.documentation.contains("```example"));
    assert!(!heading_hover.documentation.contains("@reference:"));
    assert!(heading_hover.documentation.contains("show rule"));

    // Hovering over `where` in `heading.where(...)` resolves `heading.where`.
    let where_idx = show_src.find("where").unwrap() + 2;
    let where_hover = compute_hover_info(show_src, where_idx, &[], &[])
        .expect("expected hover for heading.where");
    assert_eq!(where_hover.title, "heading.where");
    assert!(!where_hover.documentation.is_empty());
    assert!(!where_hover.documentation.contains("```example"));
    assert!(where_hover
        .documentation
        .contains("Returns a selector that filters"));
    assert!(where_hover.documentation.contains("Example:"));

    // `#heading.w` and `#{ heading.` dotted completions include `where` / `with` with sanitized docs.
    let dotted = compute_completions("#heading.w", 10, &[], &[])
        .expect("expected dotted completions for #heading.w");
    let where_item = dotted
        .items
        .iter()
        .find(|i| i.label == "where")
        .expect("expected heading.where completion");
    assert!(!where_item.documentation.contains("```example"));
    assert!(dotted.items.iter().any(|i| i.label == "with"));

    // Pure sanitizer cleans doc links, code fences, `{code}` wrappers, and extra blank lines.
    let raw = "See [show rules]($reference/styling) and @heading.body[`body`].\n\n\n\n```typ\n#set text(size: 10pt)\n```\n\n\nDone with `{false}`.";
    let sanitized = stdlib::sanitize_typst_docs(raw);
    assert_eq!(
        sanitized,
        "See show rules and `body`.\n\n  #set text(size: 10pt)\n\nDone with `false`."
    );
}
