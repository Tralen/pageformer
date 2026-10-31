//! The CSS generated from the template.

use crate::assets::{Assets, media_type};
use crate::document::Metadata;
use crate::html::data_uri::data_uri;
use crate::metrics;
use crate::template::{
    BlockStyle, FontFamily, InlineStyle, Length, LinkStyle, ListStyle, ParagraphAlignment,
    PartialTextStyle, RuleStyle, TaskMarkerStyle, Template, TextFace,
};

/// The class suffixes used for list depths. The Markdown parser rejects nesting
/// deeper than 128, so one class per level is enough for every document.
const LIST_DEPTH_CLASSES: usize = 128;

/// The stylesheet Pageweaver can use for its preview: screen rules plus the
/// structural print rules, without document metadata, running content or
/// letterhead.
pub fn stylesheet(template: &Template) -> String {
    let mut output = screen(template);
    output.push_str(&print(template, None));
    output
}

/// The complete stylesheet of one document.
pub(crate) fn document_stylesheet(
    template: &Template,
    metadata: &Metadata,
    assets: &Assets,
) -> String {
    let mut output = screen(template);
    output.push_str(&print(template, Some((metadata, assets))));
    output
}

fn screen(template: &Template) -> String {
    let page = template.page();
    let dimensions = page.dimensions();
    let margins = page.margins();
    let paragraph = template.formatting().paragraph();
    let paragraph_after = metrics::paragraph_after(paragraph.spacing(), paragraph.leading());
    let face = paragraph.face();
    let line_height = if face.size().points() > 0.0 {
        metrics::line_advance(face.size(), paragraph.leading()) / face.size().points()
    } else {
        1.0
    };

    let mut output = String::new();
    output.push_str("/* Pageformer HTML stylesheet. */\n");
    output.push_str(":root { color-scheme: light; }\n");
    output.push_str("* { box-sizing: border-box; }\n");
    output.push_str("html { background: #f2f2f2; }\n");
    output.push_str("body { margin: 0; background: #f2f2f2; }\n");

    let page_background = page
        .background()
        .map_or_else(|| "#ffffff".to_owned(), |color| color.hex());
    let mut main = vec![
        format!("width: {};", length(dimensions.width())),
        "max-width: 100%;".to_owned(),
        "margin: 24pt auto;".to_owned(),
        format!(
            "padding: {} {} {} {};",
            length(margins.top()),
            length(margins.right()),
            length(margins.bottom()),
            length(margins.left())
        ),
        format!("background: {page_background};"),
    ];
    main.extend(face_declarations(face));
    main.push(format!("line-height: {line_height:.4};"));
    main.push(format!("text-align: {};", alignment(paragraph.alignment())));
    output.push_str(&rule("main.pageformer", &main));

    output.push_str(&content_rules(template, paragraph_after));
    output.push_str(
        "@media screen and (max-width: 720pt) { main.pageformer { width: 100%; padding: 24pt; } }\n",
    );

    output
}

/// The reflowable stylesheet used by EPUB chapters.
pub(crate) fn epub_stylesheet(template: &Template) -> String {
    let paragraph = template.formatting().paragraph();
    let paragraph_after = metrics::paragraph_after(paragraph.spacing(), paragraph.leading());
    let face = paragraph.face();
    let line_height = if face.size().points() > 0.0 {
        metrics::line_advance(face.size(), paragraph.leading()) / face.size().points()
    } else {
        1.0
    };
    let mut body = face_declarations(face);
    body.push(format!("line-height: {line_height:.4};"));
    body.push(format!("text-align: {};", alignment(paragraph.alignment())));

    let mut output = String::from("/* Pageformer EPUB stylesheet. */\n");
    output.push_str("html, body { margin: 0; padding: 0; }\n");
    if let Some(background) = template.page().background() {
        output.push_str(&rule(
            "html, body",
            &[format!("background-color: {};", background.hex())],
        ));
    }
    output.push_str(&rule("body", &body));
    output.push_str("img { max-width: 100%; height: auto; }\n");
    output.push_str(&content_rules(template, paragraph_after));
    output.push_str("h1, h2, h3, h4, h5, h6 { break-before: auto; page-break-before: auto; }\n");
    output.push_str(".page-break { display: none; break-after: auto; page-break-after: auto; }\n");
    output
}

fn content_rules(template: &Template, paragraph_after: f64) -> String {
    let paragraph = template.formatting().paragraph();
    let mut output = String::new();

    output.push_str(&rule(
        "p",
        &[format!("margin: 0 0 {} 0;", points(paragraph_after))],
    ));
    if let Some(background) = paragraph.background() {
        output.push_str(&rule(
            "p.body-paragraph",
            &[format!("background-color: {};", background.hex())],
        ));
    }
    output.push_str(&rule(
        "p.numbered .number",
        &["margin-inline-end: 0.35em;".to_owned()],
    ));
    output.push_str(&rule(
        "main.pageformer > :last-child",
        &["margin-bottom: 0;".to_owned()],
    ));

    output.push_str(&inline_rules(template));

    for (index, heading) in template.formatting().heading().levels().iter().enumerate() {
        let level = index.saturating_add(1);
        let mut declarations = face_declarations(heading.face());
        let below = heading
            .rule()
            .and_then(RuleStyle::spacing_after)
            .unwrap_or_else(|| heading.spacing_after());
        declarations.push(format!(
            "margin: {} 0 {} 0;",
            length(heading.spacing_before()),
            length(below)
        ));
        declarations.push("line-height: normal;".to_owned());
        let fill = heading
            .background()
            .map_or(0.0, |_| metrics::fill_padding(heading.face().size()));
        let mut top = fill;
        let mut bottom = fill;
        if let Some(background) = heading.background() {
            declarations.push(format!("background-color: {};", background.hex()));
        }
        if let Some(rule) = heading.rule() {
            declarations.push(format!(
                "border-bottom: {} solid {};",
                length(rule.thickness()),
                rule.color().hex()
            ));
            bottom = bottom.max(rule.spacing_before().map_or(0.0, Length::points));
        }
        top = top.max(0.0);
        bottom = bottom.max(0.0);
        if top > 0.0 || bottom > 0.0 {
            declarations.push(format!("padding: {} 0 {} 0;", points(top), points(bottom)));
        }
        if heading.page_break_before() {
            declarations.push("break-before: page;".to_owned());
            declarations.push("page-break-before: always;".to_owned());
        }
        output.push_str(&rule(&format!("h{level}"), &declarations));
    }

    let horizontal = template.formatting().horizontal_rule();
    output.push_str(&rule(
        "hr.horizontal-rule",
        &[
            "border: 0;".to_owned(),
            format!(
                "border-top: {} solid {};",
                length(horizontal.thickness()),
                horizontal.color().hex()
            ),
            format!(
                "margin: {} 0 {} 0;",
                optional_length(horizontal.spacing_before()),
                optional_length(horizontal.spacing_after())
            ),
            "height: 0;".to_owned(),
        ],
    ));

    let quote = template.formatting().quote();
    output.push_str(&block_rule(
        "blockquote.quote, aside.quote",
        quote,
        paragraph_after,
        template.formatting().paragraph().face().size(),
    ));
    output.push_str(
        "blockquote.quote > :first-child, aside.quote > :first-child { margin-top: 0; }\n",
    );
    output.push_str(
        "blockquote.quote > :last-child, aside.quote > :last-child { margin-bottom: 0; }\n",
    );
    output.push_str(&rule(
        ".alert-label",
        &[
            "margin: 0 0 0.35em 0;".to_owned(),
            "font-weight: inherit;".to_owned(),
        ],
    ));

    let code_block = template.formatting().code_block();
    output.push_str(&block_rule(
        "pre.code-block",
        code_block,
        paragraph_after,
        template.formatting().paragraph().face().size(),
    ));
    output.push_str("pre.code-block { white-space: pre-wrap; overflow-wrap: anywhere; }\n");
    output.push_str("pre.code-block code { font: inherit; white-space: inherit; }\n");
    output.push_str("img { max-width: 100%; height: auto; }\n");
    output.push_str(&image_rules(template, paragraph_after));

    output.push_str(&list_rules(template, paragraph_after));
    output.push_str(&table_rules(template, paragraph_after));
    output.push_str(&footnote_rules(template, paragraph_after));
    output.push_str(&rule(
        ".page-break",
        &[
            "height: 0;".to_owned(),
            "margin: 0;".to_owned(),
            "border: 0;".to_owned(),
        ],
    ));
    output
}

fn inline_rules(template: &Template) -> String {
    let paragraph = template.formatting().paragraph();
    let mut output = String::new();
    output.push_str(&rule(
        "em.emphasis",
        &inline_declarations(paragraph.emphasis()),
    ));
    output.push_str(&rule(
        "strong.strong",
        &inline_declarations(paragraph.strong()),
    ));
    output.push_str(&rule(
        "code.inline-code",
        &partial_declarations(paragraph.code()),
    ));
    let mut strike = partial_declarations(paragraph.strikethrough());
    strike.push("text-decoration: line-through;".to_owned());
    output.push_str(&rule("s.strikethrough", &strike));
    output.push_str(&link_rule(paragraph.link()));
    output
}

fn link_rule(link: &LinkStyle) -> String {
    let mut declarations = partial_declarations(link.text());
    declarations.push(format!(
        "text-decoration: {};",
        if link.underline() {
            "underline"
        } else {
            "none"
        }
    ));
    rule("a", &declarations)
}

fn block_rule(selector: &str, style: &BlockStyle, after: f64, paragraph_size: Length) -> String {
    let mut declarations = partial_declarations(style.text());
    declarations.push(format!("text-align: {};", alignment(style.alignment())));
    let border = style.border();
    let fill = if style.text().background().is_some() {
        style.text().size().map_or_else(
            || metrics::fill_padding(paragraph_size),
            metrics::fill_padding,
        )
    } else {
        0.0
    };
    let side = |value: Option<Length>| value.map_or(0.0, |_| border.padding().points());
    let top = side(border.top()).max(fill);
    let bottom = side(border.bottom()).max(fill);
    let left = side(border.left());
    let right = side(border.right());
    if border.is_empty() {
        declarations.push("border: 0;".to_owned());
    } else {
        declarations.push("border-style: solid;".to_owned());
        declarations.push(format!("border-color: {};", border.color().hex()));
        declarations.push(format!(
            "border-width: {} {} {} {};",
            optional_side(border.top()),
            optional_side(border.right()),
            optional_side(border.bottom()),
            optional_side(border.left())
        ));
    }
    if top > 0.0 || right > 0.0 || bottom > 0.0 || left > 0.0 {
        declarations.push(format!(
            "padding: {} {} {} {};",
            points(top),
            points(right),
            points(bottom),
            points(left)
        ));
    }
    declarations.push(format!("margin: 0 0 {} 0;", points(after)));
    rule(selector, &declarations)
}

fn image_rules(template: &Template, paragraph_after: f64) -> String {
    let image = template.formatting().image();
    let border = image.border();
    let fill_padding = if image.background().is_some() || !border.is_empty() {
        border.padding().points()
    } else {
        0.0
    };
    let max_width = match image.max_width() {
        crate::image::ImageDimension::Percent(percent) => format!("{percent:.2}%"),
        crate::image::ImageDimension::Points(points) => format!("{points:.2}pt"),
    };
    let mut output = String::new();
    output.push_str(&rule(
        "figure.image-block",
        &[
            format!("margin: 0 0 {} 0;", points(paragraph_after)),
            "margin-inline-start: 0;".to_owned(),
            "margin-inline-end: 0;".to_owned(),
        ],
    ));
    for (class, align) in [
        ("align-left", "left"),
        ("align-center", "center"),
        ("align-right", "right"),
    ] {
        output.push_str(&rule(
            &format!("figure.image-block.{class}"),
            &[format!("text-align: {align};")],
        ));
    }
    output.push_str(&rule(
        "figure.image-block img",
        &[
            "display: block;".to_owned(),
            format!("max-width: {max_width};"),
            "height: auto;".to_owned(),
        ],
    ));
    let mut frame = vec![
        "display: inline-block;".to_owned(),
        "max-width: 100%;".to_owned(),
    ];
    if fill_padding > 0.0 {
        frame.push(format!("padding: {};", points(fill_padding)));
    }
    if !border.is_empty() {
        frame.push("border-style: solid;".to_owned());
        frame.push(format!("border-color: {};", border.color().hex()));
        frame.push(format!(
            "border-width: {} {} {} {};",
            optional_side(border.top()),
            optional_side(border.right()),
            optional_side(border.bottom()),
            optional_side(border.left())
        ));
    }
    if let Some(background) = image.background() {
        frame.push(format!("background-color: {};", background.hex()));
    }
    output.push_str(&rule("figure.image-block .image-frame", &frame));
    output
}

fn list_rules(template: &Template, paragraph_after: f64) -> String {
    let list = template.formatting().list();
    let task = list.task();
    let mut output = String::new();

    output.push_str(&rule(
        "ul, ol",
        &[
            format!("margin: 0 0 {} 0;", points(paragraph_after)),
            format!(
                "padding-inline-start: {};",
                points(list.indent().points() + list.marker_gap().points())
            ),
            "list-style-position: outside;".to_owned(),
        ],
    ));

    let mut item = partial_text_declarations(list.text());
    item.push(format!("text-align: {};", alignment(list.alignment())));
    output.push_str(&rule("li", &item));
    if let Some(background) = list.text().background() {
        output.push_str(&rule(
            ".list-paragraph",
            &[format!("background-color: {};", background.hex())],
        ));
    }
    output.push_str(&rule("li > p", &["margin: 0;".to_owned()]));
    output.push_str(&rule("li > ul, li > ol", &["margin-bottom: 0;".to_owned()]));
    if let Some(spacing) = list.spacing().filter(|spacing| spacing.points() > 0.0) {
        output.push_str(&rule(
            ".list-loose > li + li",
            &[format!("margin-top: {};", length(spacing))],
        ));
    }

    for (index, bullet) in list.bullets().iter().enumerate() {
        output.push_str(&format!(
            "@counter-style pf-bullet-{index} {{ system: cyclic; symbols: {}; suffix: \" \"; }}\n",
            css_string(bullet)
        ));
    }
    for (index, format) in list.numbering().iter().enumerate() {
        output.push_str(&format!(
            "@counter-style pf-number-{index} {{ system: extends {}; suffix: {}; }}\n",
            format.css_name(),
            css_string(&numbering_suffix(list))
        ));
    }
    let bullet_count = list.bullets().len().max(1);
    let number_count = list.numbering().len().max(1);
    for depth in 0..=LIST_DEPTH_CLASSES {
        let bullet = depth % bullet_count;
        let number = depth % number_count;
        output.push_str(&format!(
            "ul.list-depth-{depth} {{ list-style-type: pf-bullet-{bullet}; }}\n"
        ));
        output.push_str(&format!(
            "ol.list-depth-{depth} {{ list-style-type: pf-number-{number}; }}\n"
        ));
    }

    output.push_str(&task_rule(".task-done", task.done()));
    output.push_str(&task_rule(".task-todo", task.todo()));
    output.push_str(&rule(
        "ul.task-list, ol.task-list",
        &["list-style: none;".to_owned()],
    ));
    output
}

fn task_rule(selector: &str, marker: &TaskMarkerStyle) -> String {
    rule(selector, &partial_declarations(marker.text()))
}

fn numbering_suffix(list: &ListStyle) -> String {
    let suffix = list.suffix().trim_end();
    if suffix.is_empty() {
        " ".to_owned()
    } else {
        format!("{suffix} ")
    }
}

fn table_rules(template: &Template, paragraph_after: f64) -> String {
    let table = template.table();
    let border = if table.border().points() > 0.0 {
        format!(
            "border: {} solid {};",
            length(table.border()),
            table.border_color().hex()
        )
    } else {
        "border: 0;".to_owned()
    };
    let mut output = String::new();
    output.push_str(&rule(
        "table",
        &[
            "width: 100%;".to_owned(),
            "border-collapse: collapse;".to_owned(),
            format!("margin: 0 0 {} 0;", points(paragraph_after)),
        ],
    ));
    output.push_str(&rule(
        "th, td",
        &[
            border,
            format!("padding: {};", length(table.cell_padding())),
            "text-align: left;".to_owned(),
            "vertical-align: top;".to_owned(),
        ],
    ));
    output.push_str("th:empty::after, td:empty::after { content: \"\\00a0\"; }\n");
    output.push_str(&rule(
        "th",
        &[
            "font-weight: inherit;".to_owned(),
            "text-align: inherit;".to_owned(),
        ],
    ));
    if let Some(fill) = table.header_fill() {
        output.push_str(&rule(
            "thead th",
            &[format!("background-color: {};", fill.hex())],
        ));
    }
    output.push_str(&rule("tr", &["break-inside: avoid;".to_owned()]));
    output.push_str(&rule(".align-left", &["text-align: left;".to_owned()]));
    output.push_str(&rule(".align-center", &["text-align: center;".to_owned()]));
    output.push_str(&rule(".align-right", &["text-align: right;".to_owned()]));
    output
}

fn footnote_rules(template: &Template, paragraph_after: f64) -> String {
    let rule_style = template.formatting().horizontal_rule();
    let mut output = String::new();
    output.push_str(&rule(
        "section.footnotes",
        &[
            format!(
                "margin-top: {};",
                points(if paragraph_after > 0.0 {
                    paragraph_after
                } else {
                    12.0
                })
            ),
            format!(
                "border-top: {} solid {};",
                length(rule_style.thickness()),
                rule_style.color().hex()
            ),
            "padding-top: 6pt;".to_owned(),
        ],
    ));
    output.push_str(&rule(
        "section.footnotes ol",
        &[
            "margin: 0;".to_owned(),
            format!(
                "padding-inline-start: {};",
                length(template.formatting().list().indent())
            ),
        ],
    ));
    output.push_str(&rule(
        "section.footnotes li",
        &[format!("margin-bottom: {};", points(paragraph_after))],
    ));
    output.push_str(&rule(
        "section.footnotes li > p",
        &[format!("margin: 0 0 {} 0;", points(paragraph_after))],
    ));
    output.push_str(&rule(
        "section.footnotes li > p:last-child",
        &["margin-bottom: 0;".to_owned()],
    ));
    output.push_str(&rule(
        "a[role=\"doc-backlink\"]",
        &["text-decoration: none;".to_owned()],
    ));
    output
}

fn print(template: &Template, document: Option<(&Metadata, &Assets)>) -> String {
    let page = template.page();
    let dimensions = page.dimensions();
    let margins = page.margins();
    let paragraph = template.formatting().paragraph();
    let face = paragraph.face();
    let mut output = String::new();

    let page_background = page
        .background()
        .map_or_else(|| "#ffffff".to_owned(), |color| color.hex());
    output.push_str("@media print {\n");
    output.push_str(&format!(
        "  html, body {{ background: {page_background}; }}\n"
    ));
    output.push_str(
        "  main.pageformer { width: auto; max-width: none; margin: 0; padding: 0; background: transparent; }\n",
    );
    output.push_str("  .page-break { break-after: page; page-break-after: always; }\n");
    output.push_str("  @page {\n");
    output.push_str(&format!(
        "    size: {} {};\n",
        length(dimensions.width()),
        length(dimensions.height())
    ));
    output.push_str(&format!(
        "    margin: {} {} {} {};\n",
        length(margins.top()),
        length(margins.right()),
        length(margins.bottom()),
        length(margins.left())
    ));
    if page.background().is_some() {
        output.push_str(&format!("    background-color: {page_background};\n"));
    }

    if let Some((metadata, assets)) = document {
        if let Some(letterhead) = letterhead_uri(assets) {
            output.push_str(&format!("    background-image: url(\"{letterhead}\");\n"));
            output.push_str("    background-repeat: no-repeat;\n");
            output.push_str("    background-position: center;\n");
            output.push_str("    background-size: 100% 100%;\n");
        }
        if let Some(header) = page.header() {
            running_boxes(&mut output, "top", header.slots(), metadata, face);
        }
        if let Some(footer) = page.footer() {
            running_boxes(&mut output, "bottom", footer.slots(), metadata, face);
        }
    }

    output.push_str("  }\n");
    output.push_str("}\n");
    output
}

fn running_boxes(
    output: &mut String,
    side: &str,
    slots: &crate::template::Slots,
    metadata: &Metadata,
    face: &TextFace,
) {
    for (position, slot) in [
        ("left", slots.left()),
        ("center", slots.center()),
        ("right", slots.right()),
    ] {
        let Some(slot) = slot else {
            continue;
        };
        let Some(content) = running_content(slot, metadata) else {
            continue;
        };
        let mut declarations = vec![format!("content: {content};")];
        declarations.extend(face_declarations(face));
        output.push_str(&nested_rule(
            &format!("@{side}-{position}"),
            &declarations,
            4,
        ));
    }
}

fn running_content(slot: &str, metadata: &Metadata) -> Option<String> {
    if slot.is_empty() {
        return None;
    }
    let mut segments = Vec::new();
    let mut rest = slot;
    while let Some(open) = rest.find('{') {
        if open > 0 {
            segments.push(css_string(&rest[..open]));
        }
        let after = &rest[open..];
        let Some(close) = after.find('}') else {
            segments.push(css_string(after));
            break;
        };
        let token = &after[1..close];
        match token {
            "page" => segments.push("counter(page)".to_owned()),
            "pages" => segments.push("counter(pages)".to_owned()),
            "title" => {
                let title = metadata.title().map_or("", |title| title.as_str());
                segments.push(css_string(title));
            }
            "author" => {
                let author = metadata.author().map_or("", |author| author.as_str());
                segments.push(css_string(author));
            }
            _ => segments.push(css_string(&after[..=close])),
        }
        rest = &after[close.saturating_add(1)..];
    }
    if !rest.is_empty() {
        segments.push(css_string(rest));
    }
    (!segments.is_empty()).then(|| segments.join(" "))
}

fn letterhead_uri(assets: &Assets) -> Option<String> {
    let name = assets.map().letterhead()?;
    let bytes = assets.files().get(name)?;
    let media = media_type(name)?;
    Some(data_uri(media, bytes))
}

fn rule(selector: &str, declarations: &[String]) -> String {
    let mut output = String::new();
    output.push_str(selector);
    output.push_str(" {\n");
    for declaration in declarations {
        output.push_str("  ");
        output.push_str(declaration);
        output.push('\n');
    }
    output.push_str("}\n");
    output
}

fn nested_rule(selector: &str, declarations: &[String], indent: usize) -> String {
    let padding = " ".repeat(indent);
    let mut output = String::new();
    output.push_str(&padding);
    output.push_str(selector);
    output.push_str(" {\n");
    for declaration in declarations {
        output.push_str(&padding);
        output.push_str("  ");
        output.push_str(declaration);
        output.push('\n');
    }
    output.push_str(&padding);
    output.push_str("}\n");
    output
}

fn face_declarations(face: &TextFace) -> Vec<String> {
    let mut declarations = vec![
        format!("font-family: {};", font_families(face.font_families())),
        format!("font-size: {};", length(face.size())),
        format!("font-weight: {};", face.weight().get()),
        format!("font-style: {};", face.slant().as_str()),
    ];
    if let Some(foreground) = face.foreground() {
        declarations.push(format!("color: {};", foreground.hex()));
    }
    declarations
}

fn partial_declarations(style: &PartialTextStyle) -> Vec<String> {
    let mut declarations = partial_text_declarations(style);
    if let Some(background) = style.background() {
        declarations.push(format!("background-color: {};", background.hex()));
    }
    declarations
}

fn partial_text_declarations(style: &PartialTextStyle) -> Vec<String> {
    let mut declarations = Vec::new();
    if let Some(families) = style.font_families() {
        declarations.push(format!("font-family: {};", font_families(families)));
    }
    if let Some(size) = style.size() {
        declarations.push(format!("font-size: {};", length(size)));
    }
    if let Some(weight) = style.weight() {
        declarations.push(format!("font-weight: {};", weight.get()));
    }
    if let Some(slant) = style.slant() {
        declarations.push(format!("font-style: {};", slant.as_str()));
    }
    if let Some(foreground) = style.foreground() {
        declarations.push(format!("color: {};", foreground.hex()));
    }
    declarations
}

fn inline_declarations(style: &InlineStyle) -> Vec<String> {
    let mut declarations = face_declarations(style.face());
    if let Some(background) = style.background() {
        declarations.push(format!("background-color: {};", background.hex()));
    }
    declarations
}

fn font_families(families: &[FontFamily]) -> String {
    families
        .iter()
        .map(|family| css_string(family.as_str()))
        .collect::<Vec<_>>()
        .join(", ")
}

fn alignment(alignment: ParagraphAlignment) -> &'static str {
    match alignment {
        ParagraphAlignment::Justified => "justify",
        ParagraphAlignment::Left => "left",
    }
}

fn length(length: Length) -> String {
    points(length.points())
}

fn points(points: f64) -> String {
    format!("{points:.2}pt")
}

fn optional_length(length: Option<Length>) -> String {
    length.map_or_else(|| "0.65em".to_owned(), |length| points(length.points()))
}

fn optional_side(length: Option<Length>) -> String {
    length.map_or_else(|| "0".to_owned(), |length| points(length.points()))
}

/// Escapes a CSS string, including `<`, `>` and `&` so the value cannot close
/// the style element or the CDATA section.
fn css_string(value: &str) -> String {
    let mut output = String::with_capacity(value.len().saturating_add(2));
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\a "),
            '\r' => output.push_str("\\d "),
            '\u{c}' => output.push_str("\\c "),
            '<' => output.push_str("\\3c "),
            '>' => output.push_str("\\3e "),
            '&' => output.push_str("\\26 "),
            character if character.is_control() => {
                output.push_str(&format!("\\{:x} ", u32::from(character)));
            }
            character => output.push(character),
        }
    }
    output.push('"');
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Author, Language, Metadata, Title};
    use crate::template::Template;

    fn template() -> Template {
        Template::from_yaml(
            "version: \"0.7\"\n\
             page:\n\
             \x20 header:\n\
             \x20   left: \"{title}\"\n\
             \x20   right: \"Page {page} of {pages}\"\n\
             formatting:\n\
             \x20 paragraph:\n\
             \x20   font_families:\n\
             \x20     - \"Liberation Serif\"\n\
             \x20   foreground: \"#222222\"\n\
             \x20   background: \"#fdfdfd\"\n\
             \x20   spacing: 24\n\
             \x20 list:\n\
             \x20   background: \"#f5f7fa\"\n\
             \x20   bullet:\n\
             \x20     - \"•\"\n\
             \x20   numbering:\n\
             \x20     format:\n\
             \x20       - decimal\n\
             \x20     suffix: \".\"\n",
        )
        .expect("a valid template")
    }

    fn metadata() -> Metadata {
        Metadata::new(
            Some(Title::parse("Contrato").expect("a title")),
            Some(Author::parse("Pageformer").expect("an author")),
            Some(Language::parse("pt-BR").expect("a language")),
        )
    }

    #[test]
    fn the_screen_style_uses_the_template_face_and_spacing() {
        let css = stylesheet(&template());

        assert!(css.contains("font-family: \"Liberation Serif\";"), "{css}");
        assert!(css.contains("color: #222222;"), "{css}");
        assert!(css.contains("background-color: #fdfdfd;"), "{css}");
        assert!(css.contains("text-align: justify;"), "{css}");
        assert!(css.contains("list-style-type: pf-bullet-0;"), "{css}");
        assert!(css.contains("list-style-type: pf-number-0;"), "{css}");
        assert!(
            css.contains("ul.task-list, ol.task-list {\n  list-style: none;\n}"),
            "{css}"
        );
        assert!(
            css.contains(".list-paragraph {\n  background-color: #f5f7fa;\n}"),
            "{css}"
        );
        assert!(css.contains("th:empty::after, td:empty::after"), "{css}");
    }

    #[test]
    fn the_print_style_carries_the_page_size_and_the_running_content() {
        let template = template();
        let assets = Assets::default();
        let css = document_stylesheet(&template, &metadata(), &assets);

        assert!(css.contains("size: 595.28pt 841.89pt;"), "{css}");
        assert!(css.contains("@top-left"), "{css}");
        assert!(css.contains("content: \"Contrato\";"), "{css}");
        assert!(
            css.contains("content: \"Page \" counter(page) \" of \" counter(pages);"),
            "{css}"
        );
    }

    #[test]
    fn a_css_string_cannot_close_the_style_or_cdata() {
        assert_eq!(
            css_string("a\"b\\c</style>]]&"),
            "\"a\\\"b\\\\c\\3c /style\\3e ]]\\26 \""
        );
    }

    #[test]
    fn the_epub_style_is_reflowable() {
        let css = epub_stylesheet(&template());

        assert!(
            css.contains("img { max-width: 100%; height: auto; }"),
            "{css}"
        );
        assert!(
            css.contains("h1, h2, h3, h4, h5, h6 { break-before: auto;"),
            "{css}"
        );
        assert!(css.contains(".page-break { display: none;"), "{css}");
        assert!(!css.contains("@page"), "{css}");
        assert!(!css.contains("main.pageformer {\n  width:"), "{css}");
    }

    #[test]
    fn deeply_nested_lists_have_a_style_for_every_allowed_depth() {
        let css = stylesheet(&template());

        assert!(css.contains("ul.list-depth-0 { list-style-type: pf-bullet-0; }"));
        assert!(css.contains("ol.list-depth-128 { list-style-type: pf-number-0; }"));
    }
}
