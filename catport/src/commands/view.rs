// use crate::parser::Cli;
use std::fs;
use std::path::PathBuf;
use crate::commands::highlighter::{apply_syntax_highlight, plain_text_highlight};
pub fn execute_view(file: PathBuf, plain: bool) -> Result<String, String> {
    if !plain {
        let format = file
            .extension()
            .unwrap_or("".as_ref())
            .to_str()
            .unwrap_or("");

        let file_cpy = file.clone();
        let file_content = fs::read_to_string(file_cpy).unwrap();

        apply_syntax_highlight(file_content.as_str(), file.to_str().unwrap());
    }
    else {
        let file_content = fs::read_to_string(file).unwrap();
        plain_text_highlight(file_content.as_str());
    }
    Ok("Success!".to_string())
}
