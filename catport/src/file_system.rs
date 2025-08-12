use std::path::PathBuf;
use std::fs;
use crate::parser::Cli;

pub fn execute_view(file: PathBuf) -> Result<String, String> {
    let format = file.extension().unwrap_or("".as_ref()).to_str().unwrap_or("");

    if format == "rs" {
        println!("File format: Rust (.rs)");
    } else {
        println!("File format: Unknown (.{})", format);
    }

    let file = fs::read_to_string(file).unwrap();

    println!("File read: {:?}", file);

    Ok("Success!".to_string())
}