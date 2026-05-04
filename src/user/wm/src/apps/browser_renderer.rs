extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use libui::canvas::Canvas;
use libui::color::PANEL_TEXT;
use libui::geom::Rect;
use libui::text::{draw_text, CHAR_W};

const PAGE_BG: u32 = 0x00f7f7f7;
const CARD_BG: u32 = 0x00ffffff;
const CARD_BORDER: u32 = 0x00b8b8b8;
const SOFT_BORDER: u32 = 0x00dddddd;
const HEADING: u32 = 0x001f2937;
const BODY: u32 = 0x00374451;
const MUTED: u32 = 0x006b7280;
const LINK: u32 = 0x000064c8;
const CODE_BG: u32 = 0x00eeeeee;
const RULE: u32 = 0x00c9c9c9;

#[derive(Clone)]
pub struct RenderedDocument {
    display_list: Vec<DisplayCommand>,
    pub content_height: i32,
}

#[derive(Clone)]
enum DisplayCommand {
    SolidColor {
        rect: Rect,
        color: u32,
    },
    Text {
        x: i32,
        y: i32,
        color: u32,
        text: String,
    },
}

#[derive(Clone)]
struct Node {
    children: Vec<Node>,
    node_type: NodeType,
}

#[derive(Clone)]
enum NodeType {
    Text(String),
    Element(ElementData),
}

#[derive(Clone)]
struct ElementData {
    tag_name: String,
    attrs: Vec<Attr>,
}

#[derive(Clone)]
struct Attr {
    name: String,
    value: String,
}

struct Parser<'a> {
    input: &'a [u8],
    pos: usize,
}

struct LayoutState {
    list: Vec<DisplayCommand>,
    y: i32,
    max_cols: usize,
    viewport_w: i32,
    card_x: i32,
    card_w: i32,
}

impl RenderedDocument {
    pub fn empty(viewport_w: i32) -> Self {
        render_plain_text("(empty page)", viewport_w)
    }

    pub fn from_text(text: &str, viewport_w: i32) -> Self {
        render_plain_text(text, viewport_w)
    }
}

pub fn render_http_response(bytes: &[u8], viewport_w: i32) -> RenderedDocument {
    let body = split_http_body(bytes);
    let root = parse_html(body);
    render_dom(&root, viewport_w)
}

pub fn paint_document(
    canvas: &mut Canvas,
    viewport: Rect,
    document: &RenderedDocument,
    scroll_y: i32,
) {
    canvas.fill_rect(viewport, PAGE_BG);

    for item in &document.display_list {
        match item {
            DisplayCommand::SolidColor { rect, color } => {
                let shifted = Rect::new(
                    viewport.x + rect.x,
                    viewport.y + rect.y - scroll_y,
                    rect.w,
                    rect.h,
                );

                if let Some(clipped) = clip_rect(shifted, viewport) {
                    canvas.fill_rect(clipped, *color);
                }
            }
            DisplayCommand::Text { x, y, color, text } => {
                let yy = viewport.y + *y - scroll_y;

                if yy + 10 < viewport.y || yy > viewport.bottom() {
                    continue;
                }

                draw_text(canvas, viewport.x + *x, yy, *color, None, text);
            }
        }
    }
}

fn render_plain_text(text: &str, viewport_w: i32) -> RenderedDocument {
    let mut state = new_layout_state(viewport_w);
    begin_card(&mut state);

    let text_x = state.card_x + 10;
    push_wrapped_block(&mut state, text, text_x, BODY, 8, 6);

    finish_card(&mut state);

    RenderedDocument {
        display_list: state.list,
        content_height: state.y + 12,
    }
}

fn render_dom(root: &Node, viewport_w: i32) -> RenderedDocument {
    let mut state = new_layout_state(viewport_w);
    begin_card(&mut state);

    render_node(root, &mut state);

    if state.list.len() <= 3 {
        let text_x = state.card_x + 10;
        push_wrapped_block(&mut state, "(empty page)", text_x, BODY, 8, 6);
    }

    finish_card(&mut state);

    RenderedDocument {
        display_list: state.list,
        content_height: state.y + 12,
    }
}

fn new_layout_state(viewport_w: i32) -> LayoutState {
    let card_x = 10;
    let card_w = viewport_w.saturating_sub(20).max(80);
    let text_w = card_w.saturating_sub(24);
    let max_cols = (text_w / CHAR_W).max(8) as usize;

    LayoutState {
        list: Vec::new(),
        y: 10,
        max_cols,
        viewport_w,
        card_x,
        card_w,
    }
}

fn begin_card(state: &mut LayoutState) {
    state.list.push(DisplayCommand::SolidColor {
        rect: Rect::new(state.card_x, 8, state.card_w, 8192),
        color: CARD_BG,
    });

    state.list.push(DisplayCommand::SolidColor {
        rect: Rect::new(state.card_x, 8, state.card_w, 1),
        color: CARD_BORDER,
    });

    state.list.push(DisplayCommand::SolidColor {
        rect: Rect::new(state.card_x, 8, 1, 8192),
        color: CARD_BORDER,
    });

    state.list.push(DisplayCommand::SolidColor {
        rect: Rect::new(state.card_x + state.card_w - 1, 8, 1, 8192),
        color: CARD_BORDER,
    });

    state.y = 18;
}

fn finish_card(state: &mut LayoutState) {
    state.list.push(DisplayCommand::SolidColor {
        rect: Rect::new(state.card_x, state.y + 6, state.card_w, 1),
        color: CARD_BORDER,
    });

    state.y += 16;
}

fn render_node(node: &Node, state: &mut LayoutState) {
    match &node.node_type {
        NodeType::Text(text) => {
            push_wrapped_block(state, text, state.card_x + 10, BODY, 0, 2);
        }
        NodeType::Element(elem) => {
            render_element(elem, &node.children, state);
        }
    }
}

fn render_element(elem: &ElementData, children: &[Node], state: &mut LayoutState) {
    let tag = elem.tag_name.as_str();

    if is_hidden_tag(tag) || elem_has_display_none(elem) {
        return;
    }

    match tag {
        "html" | "body" | "main" | "article" | "section" => {
            for child in children {
                render_node(child, state);
            }
        }

        "head" | "script" | "style" | "noscript" | "template" => {}

        "br" => {
            state.y += 12;
        }

        "hr" => {
            state.y += 6;
            state.list.push(DisplayCommand::SolidColor {
                rect: Rect::new(
                    state.card_x + 10,
                    state.y,
                    state.card_w.saturating_sub(20),
                    1,
                ),
                color: RULE,
            });
            state.y += 14;
        }

        "h1" => {
            let mut text = String::new();
            collect_text(children, &mut text);

            push_wrapped_block(state, &text, state.card_x + 10, HEADING, 4, 4);
            draw_under_rule(state, 8);
        }

        "h2" => {
            let mut text = String::new();
            collect_text(children, &mut text);

            push_wrapped_block(state, &text, state.card_x + 10, HEADING, 8, 4);
            draw_under_rule(state, 6);
        }

        "h3" | "h4" | "h5" | "h6" => {
            let mut text = String::new();
            collect_text(children, &mut text);

            push_wrapped_block(state, &text, state.card_x + 10, HEADING, 8, 4);
        }

        "p" => {
            let mut text = String::new();
            collect_text(children, &mut text);
            push_wrapped_block(state, &text, state.card_x + 10, BODY, 2, 8);
        }

        "li" => {
            let mut text = String::from("- ");
            collect_text(children, &mut text);
            push_wrapped_block(state, &text, state.card_x + 22, BODY, 0, 4);
        }

        "ul" | "ol" => {
            state.y += 2;
            for child in children {
                render_node(child, state);
            }
            state.y += 6;
        }

        "pre" => {
            let mut text = String::new();
            collect_text_preserve(children, &mut text);
            push_code_block(state, &text);
        }

        "code" => {
            let mut text = String::new();
            collect_text_preserve(children, &mut text);
            push_inline_code_block(state, &text);
        }

        "a" => {
            let mut text = String::new();
            collect_text(children, &mut text);

            if text.is_empty() {
                if let Some(href) = elem.attr("href") {
                    text.push_str(href);
                }
            }

            push_wrapped_block(state, &text, state.card_x + 10, LINK, 0, 2);
        }

        "em" | "strong" | "b" | "i" | "span" => {
            let mut text = String::new();
            collect_text(children, &mut text);
            push_wrapped_block(state, &text, state.card_x + 10, BODY, 0, 2);
        }

        "img" => {
            let label = match elem.attr("alt") {
                Some(alt) if !alt.is_empty() => alt,
                _ => "[image]",
            };

            draw_placeholder_box(state, label);
        }

        "blockquote" => {
            let mut text = String::new();
            collect_text(children, &mut text);
            push_quote_block(state, &text);
        }

        _ => {
            let before_y = state.y;

            for child in children {
                render_node(child, state);
            }

            if state.y == before_y {
                let mut text = String::new();
                collect_text(children, &mut text);

                if !trim_ascii_str(&text).is_empty() {
                    push_wrapped_block(state, &text, state.card_x + 10, BODY, 0, 6);
                }
            }
        }
    }
}

fn draw_under_rule(state: &mut LayoutState, bottom_gap: i32) {
    state.list.push(DisplayCommand::SolidColor {
        rect: Rect::new(
            state.card_x + 10,
            state.y + 1,
            state.card_w.saturating_sub(20),
            1,
        ),
        color: SOFT_BORDER,
    });

    state.y += bottom_gap;
}

fn draw_placeholder_box(state: &mut LayoutState, label: &str) {
    let x = state.card_x + 10;
    let w = state.card_w.saturating_sub(20);
    let h = 42;

    state.y += 4;

    state.list.push(DisplayCommand::SolidColor {
        rect: Rect::new(x, state.y, w, h),
        color: CODE_BG,
    });

    state.list.push(DisplayCommand::SolidColor {
        rect: Rect::new(x, state.y, w, 1),
        color: SOFT_BORDER,
    });

    state.list.push(DisplayCommand::Text {
        x: x + 8,
        y: state.y + 16,
        color: MUTED,
        text: String::from(label),
    });

    state.y += h + 8;
}

fn push_quote_block(state: &mut LayoutState, text: &str) {
    let x = state.card_x + 18;

    state.y += 4;

    state.list.push(DisplayCommand::SolidColor {
        rect: Rect::new(state.card_x + 10, state.y, 3, 42),
        color: SOFT_BORDER,
    });

    push_wrapped_block(state, text, x, MUTED, 0, 8);
}

fn push_code_block(state: &mut LayoutState, text: &str) {
    let x = state.card_x + 10;
    let y0 = state.y + 4;

    let lines = count_lines(text).max(1);
    let h = (lines as i32 * 14) + 12;

    state.list.push(DisplayCommand::SolidColor {
        rect: Rect::new(x, y0, state.card_w.saturating_sub(20), h),
        color: CODE_BG,
    });

    state.list.push(DisplayCommand::SolidColor {
        rect: Rect::new(x, y0, state.card_w.saturating_sub(20), 1),
        color: SOFT_BORDER,
    });

    state.y = y0 + 6;
    push_pre_block(state, text, x + 8, PANEL_TEXT);
    state.y += 8;
}

fn push_inline_code_block(state: &mut LayoutState, text: &str) {
    let clean = collapse_ascii_whitespace(text);
    let clean = trim_ascii_str(&clean);

    if clean.is_empty() {
        return;
    }

    let x = state.card_x + 10;
    let y = state.y + 2;
    let w = state.card_w.saturating_sub(20);
    let h = 20;

    state.list.push(DisplayCommand::SolidColor {
        rect: Rect::new(x, y, w, h),
        color: CODE_BG,
    });

    state.list.push(DisplayCommand::Text {
        x: x + 6,
        y: y + 6,
        color: PANEL_TEXT,
        text: trim_to_cols(clean, state.max_cols.saturating_sub(2)),
    });

    state.y += h + 6;
}

fn push_wrapped_block(
    state: &mut LayoutState,
    text: &str,
    x: i32,
    color: u32,
    top_gap: i32,
    bottom_gap: i32,
) {
    let clean = collapse_ascii_whitespace(text);
    let clean = trim_ascii_str(&clean);

    if clean.is_empty() {
        return;
    }

    state.y += top_gap;

    let max_cols = state
        .max_cols
        .saturating_sub(((x - state.card_x) / CHAR_W).max(0) as usize);
    let lines = wrap_text(clean, max_cols.max(8));

    for line in lines {
        state.list.push(DisplayCommand::Text {
            x,
            y: state.y,
            color,
            text: line,
        });

        state.y += 14;
    }

    state.y += bottom_gap;
}

fn push_pre_block(state: &mut LayoutState, text: &str, x: i32, color: u32) {
    let max_cols = state
        .max_cols
        .saturating_sub(((x - state.card_x) / CHAR_W).max(0) as usize);

    for raw_line in text.split('\n') {
        let mut line = String::new();

        for b in raw_line.bytes() {
            match b {
                0x20..=0x7e => line.push(b as char),
                b'\t' => line.push_str("    "),
                _ => {}
            }

            if line.len() >= max_cols.max(8) {
                state.list.push(DisplayCommand::Text {
                    x,
                    y: state.y,
                    color,
                    text: line,
                });

                state.y += 14;
                line = String::new();
            }
        }

        state.list.push(DisplayCommand::Text {
            x,
            y: state.y,
            color,
            text: line,
        });

        state.y += 14;
    }
}

fn wrap_text(text: &str, max_cols: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();

    for word in text.split(' ') {
        if word.is_empty() {
            continue;
        }

        if current.is_empty() {
            if word.len() <= max_cols {
                current.push_str(word);
            } else {
                push_long_word(&mut lines, word, max_cols);
            }

            continue;
        }

        let projected = current.len() + 1 + word.len();

        if projected <= max_cols {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(current);
            current = String::new();

            if word.len() <= max_cols {
                current.push_str(word);
            } else {
                push_long_word(&mut lines, word, max_cols);
            }
        }
    }

    if !current.is_empty() {
        lines.push(current);
    }

    lines
}

fn push_long_word(lines: &mut Vec<String>, word: &str, max_cols: usize) {
    let bytes = word.as_bytes();
    let mut i = 0usize;

    while i < bytes.len() {
        let end = core::cmp::min(i + max_cols.max(8), bytes.len());
        let mut part = String::new();

        for b in &bytes[i..end] {
            if *b >= 0x20 && *b <= 0x7e {
                part.push(*b as char);
            }
        }

        if !part.is_empty() {
            lines.push(part);
        }

        i = end;
    }
}

fn collect_text(nodes: &[Node], out: &mut String) {
    for node in nodes {
        match &node.node_type {
            NodeType::Text(text) => {
                if !out.is_empty() {
                    out.push(' ');
                }

                out.push_str(text);
            }
            NodeType::Element(elem) => {
                let tag = elem.tag_name.as_str();

                if is_hidden_tag(tag) || elem_has_display_none(elem) {
                    continue;
                }

                match tag {
                    "br" => out.push('\n'),
                    "img" => {
                        if let Some(alt) = elem.attr("alt") {
                            if !alt.is_empty() {
                                out.push_str(alt);
                            }
                        }
                    }
                    _ => collect_text(&node.children, out),
                }
            }
        }
    }
}

fn collect_text_preserve(nodes: &[Node], out: &mut String) {
    for node in nodes {
        match &node.node_type {
            NodeType::Text(text) => out.push_str(text),
            NodeType::Element(elem) => {
                if is_hidden_tag(elem.tag_name.as_str()) || elem_has_display_none(elem) {
                    continue;
                }

                collect_text_preserve(&node.children, out);
            }
        }
    }
}

fn is_hidden_tag(tag: &str) -> bool {
    tag == "head"
        || tag == "script"
        || tag == "style"
        || tag == "template"
        || tag == "meta"
        || tag == "link"
        || tag == "title"
}

fn elem_has_display_none(elem: &ElementData) -> bool {
    let Some(style) = elem.attr("style") else {
        return false;
    };

    contains_ci(style.as_bytes(), b"display:none")
        || contains_ci(style.as_bytes(), b"display: none")
}

fn split_http_body(bytes: &[u8]) -> &[u8] {
    let mut i = 0usize;

    while i + 3 < bytes.len() {
        if bytes[i] == b'\r'
            && bytes[i + 1] == b'\n'
            && bytes[i + 2] == b'\r'
            && bytes[i + 3] == b'\n'
        {
            return &bytes[i + 4..];
        }

        i += 1;
    }

    bytes
}

fn parse_html(bytes: &[u8]) -> Node {
    let mut parser = Parser {
        input: bytes,
        pos: 0,
    };

    let children = parser.parse_nodes();

    Node {
        children,
        node_type: NodeType::Element(ElementData {
            tag_name: String::from("html"),
            attrs: Vec::new(),
        }),
    }
}

impl<'a> Parser<'a> {
    fn parse_nodes(&mut self) -> Vec<Node> {
        let mut nodes = Vec::new();

        while !self.eof() {
            if self.starts_with(b"</") {
                break;
            }

            if self.starts_with(b"<!--") {
                self.skip_comment();
                continue;
            }

            if self.starts_with(b"<!") {
                self.skip_until_after(b'>');
                continue;
            }

            if self.starts_with(b"<?") {
                self.skip_until_after(b'>');
                continue;
            }

            let node = if self.peek() == Some(b'<') {
                self.parse_element()
            } else {
                self.parse_text()
            };

            nodes.push(node);
        }

        nodes
    }

    fn parse_element(&mut self) -> Node {
        self.consume_byte();

        let tag_name = self.parse_name();

        if tag_name.is_empty() {
            self.skip_until_after(b'>');
            return Node {
                children: Vec::new(),
                node_type: NodeType::Text(String::new()),
            };
        }

        let attrs = self.parse_attrs();

        let self_closing = self.starts_with(b"/>");
        if self_closing {
            self.pos += 2;
        } else if self.peek() == Some(b'>') {
            self.consume_byte();
        }

        let children = if self_closing || is_void_tag(tag_name.as_str()) {
            Vec::new()
        } else {
            let children = self.parse_nodes();
            self.consume_close_tag(tag_name.as_str());
            children
        };

        Node {
            children,
            node_type: NodeType::Element(ElementData { tag_name, attrs }),
        }
    }

    fn parse_text(&mut self) -> Node {
        let mut out = String::new();

        while !self.eof() {
            if self.peek() == Some(b'<') {
                break;
            }

            let b = self.consume_byte();

            if b == b'&' {
                self.consume_entity(&mut out);
                continue;
            }

            match b {
                b'\r' | b'\n' | b'\t' | b' ' => out.push(' '),
                0x20..=0x7e => out.push(b as char),
                _ => {}
            }
        }

        Node {
            children: Vec::new(),
            node_type: NodeType::Text(out),
        }
    }

    fn parse_attrs(&mut self) -> Vec<Attr> {
        let mut attrs = Vec::new();

        loop {
            self.consume_whitespace();

            if self.eof() || self.peek() == Some(b'>') || self.starts_with(b"/>") {
                break;
            }

            let name = self.parse_name();

            if name.is_empty() {
                self.consume_byte();
                continue;
            }

            self.consume_whitespace();

            let mut value = String::new();

            if self.peek() == Some(b'=') {
                self.consume_byte();
                self.consume_whitespace();
                value = self.parse_attr_value();
            }

            attrs.push(Attr { name, value });
        }

        attrs
    }

    fn parse_attr_value(&mut self) -> String {
        let mut out = String::new();

        if self.eof() {
            return out;
        }

        let quote = self.peek().unwrap_or(0);

        if quote == b'"' || quote == b'\'' {
            self.consume_byte();

            while !self.eof() && self.peek() != Some(quote) {
                let b = self.consume_byte();

                if b == b'&' {
                    self.consume_entity(&mut out);
                    continue;
                }

                if b >= 0x20 && b <= 0x7e {
                    out.push(b as char);
                }
            }

            if self.peek() == Some(quote) {
                self.consume_byte();
            }

            return out;
        }

        while !self.eof() {
            let Some(b) = self.peek() else {
                break;
            };

            if is_ascii_space(b) || b == b'>' {
                break;
            }

            let b = self.consume_byte();
            if b >= 0x20 && b <= 0x7e {
                out.push(b as char);
            }
        }

        out
    }

    fn parse_name(&mut self) -> String {
        let mut out = String::new();

        while !self.eof() {
            let Some(b) = self.peek() else {
                break;
            };

            if is_name_char(b) {
                out.push(to_ascii_lower(b) as char);
                self.consume_byte();
            } else {
                break;
            }
        }

        out
    }

    fn consume_close_tag(&mut self, tag_name: &str) {
        if !self.starts_with(b"</") {
            return;
        }

        self.pos += 2;
        self.consume_whitespace();

        let close_name = self.parse_name();

        if close_name != tag_name {
            self.skip_until_after(b'>');
            return;
        }

        self.consume_whitespace();

        if self.peek() == Some(b'>') {
            self.consume_byte();
        }
    }

    fn consume_entity(&mut self, out: &mut String) {
        if self.starts_with(b"amp;") {
            self.pos += 4;
            out.push('&');
        } else if self.starts_with(b"lt;") {
            self.pos += 3;
            out.push('<');
        } else if self.starts_with(b"gt;") {
            self.pos += 3;
            out.push('>');
        } else if self.starts_with(b"quot;") {
            self.pos += 5;
            out.push('"');
        } else if self.starts_with(b"#39;") {
            self.pos += 4;
            out.push('\'');
        } else if self.starts_with(b"nbsp;") {
            self.pos += 5;
            out.push(' ');
        } else {
            out.push('&');
        }
    }

    fn skip_comment(&mut self) {
        self.pos += 4;

        while !self.eof() {
            if self.starts_with(b"-->") {
                self.pos += 3;
                return;
            }

            self.consume_byte();
        }
    }

    fn skip_until_after(&mut self, target: u8) {
        while !self.eof() {
            let b = self.consume_byte();

            if b == target {
                break;
            }
        }
    }

    fn consume_whitespace(&mut self) {
        while !self.eof() {
            let Some(b) = self.peek() else {
                break;
            };

            if !is_ascii_space(b) {
                break;
            }

            self.consume_byte();
        }
    }

    fn starts_with(&self, s: &[u8]) -> bool {
        if self.pos + s.len() > self.input.len() {
            return false;
        }

        &self.input[self.pos..self.pos + s.len()] == s
    }

    fn peek(&self) -> Option<u8> {
        if self.pos >= self.input.len() {
            None
        } else {
            Some(self.input[self.pos])
        }
    }

    fn consume_byte(&mut self) -> u8 {
        let b = self.input[self.pos];
        self.pos += 1;
        b
    }

    fn eof(&self) -> bool {
        self.pos >= self.input.len()
    }
}

impl ElementData {
    fn attr(&self, name: &str) -> Option<&str> {
        for attr in &self.attrs {
            if attr.name == name {
                return Some(attr.value.as_str());
            }
        }

        None
    }
}

fn is_void_tag(tag: &str) -> bool {
    tag == "area"
        || tag == "base"
        || tag == "br"
        || tag == "col"
        || tag == "embed"
        || tag == "hr"
        || tag == "img"
        || tag == "input"
        || tag == "link"
        || tag == "meta"
        || tag == "param"
        || tag == "source"
        || tag == "track"
        || tag == "wbr"
}

fn collapse_ascii_whitespace(s: &str) -> String {
    let mut out = String::new();
    let mut pending_space = false;

    for b in s.bytes() {
        match b {
            b'\r' | b'\n' | b'\t' | b' ' => {
                pending_space = true;
            }
            0x20..=0x7e => {
                if pending_space && !out.is_empty() {
                    out.push(' ');
                }

                out.push(b as char);
                pending_space = false;
            }
            _ => {}
        }
    }

    out
}

fn trim_ascii_str(s: &str) -> &str {
    let bytes = s.as_bytes();
    let mut start = 0usize;
    let mut end = bytes.len();

    while start < end && is_ascii_space(bytes[start]) {
        start += 1;
    }

    while end > start && is_ascii_space(bytes[end - 1]) {
        end -= 1;
    }

    &s[start..end]
}

fn trim_to_cols(s: &str, max_cols: usize) -> String {
    let mut out = String::new();

    for b in s.bytes().take(max_cols) {
        if b >= 0x20 && b <= 0x7e {
            out.push(b as char);
        }
    }

    out
}

fn count_lines(s: &str) -> usize {
    let mut count = 1usize;

    for b in s.bytes() {
        if b == b'\n' {
            count += 1;
        }
    }

    count
}

fn clip_rect(rect: Rect, clip: Rect) -> Option<Rect> {
    let x0 = rect.x.max(clip.x);
    let y0 = rect.y.max(clip.y);
    let x1 = rect.right().min(clip.right());
    let y1 = rect.bottom().min(clip.bottom());

    if x1 <= x0 || y1 <= y0 {
        None
    } else {
        Some(Rect::new(x0, y0, x1 - x0, y1 - y0))
    }
}

fn contains_ci(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() {
        return true;
    }

    if haystack.len() < needle.len() {
        return false;
    }

    let mut i = 0usize;
    while i + needle.len() <= haystack.len() {
        let mut ok = true;

        for j in 0..needle.len() {
            if to_ascii_lower(haystack[i + j]) != to_ascii_lower(needle[j]) {
                ok = false;
                break;
            }
        }

        if ok {
            return true;
        }

        i += 1;
    }

    false
}

fn is_ascii_space(b: u8) -> bool {
    b == b' ' || b == b'\n' || b == b'\r' || b == b'\t' || b == 0x0c
}

fn is_name_char(b: u8) -> bool {
    (b'a' <= b && b <= b'z')
        || (b'A' <= b && b <= b'Z')
        || (b'0' <= b && b <= b'9')
        || b == b'-'
        || b == b'_'
        || b == b':'
}

fn to_ascii_lower(b: u8) -> u8 {
    if b'A' <= b && b <= b'Z' {
        b + 32
    } else {
        b
    }
}
