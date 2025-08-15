use syntect::{
    easy::HighlightLines,
    highlighting::*,
    parsing::SyntaxSet,
    util::{LinesWithEndings, as_24_bit_terminal_escaped},
};

pub fn apply_syntax_highlight(content: &str, file_path: &str) {
    let ss = SyntaxSet::load_defaults_newlines();
    let ts = ThemeSet::load_defaults();

    let syntax = ss
        .find_syntax_for_file(file_path)
        .unwrap()
        .unwrap_or_else(|| ss.find_syntax_plain_text());

    let mut h = HighlightLines::new(syntax, &ts.themes["base16-ocean.dark"]);
    let s = content;

    for (index, line) in LinesWithEndings::from(s).enumerate() {
        let ranges: Vec<(Style, &str)> = h.highlight_line(line, &ss).unwrap();
        let escaped = as_24_bit_terminal_escaped(&ranges[..], true);
        print!("{}  {}", index + 1, escaped);
    }
}

pub fn plain_text_highlight(file_path: &str) {
    for (index, line) in LinesWithEndings::from(file_path).enumerate() {
        print!("{}  {}", index, line);
    }
}
