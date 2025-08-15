// use serde::{Serialize, Deserialize, Serializer};
use std::path::PathBuf;
pub async fn start_sharing(file: PathBuf) {
    make_json_from_file(file);
    let listener = tokio::net::TcpListener::bind("0.0.0.0:0").await.unwrap();
}

pub fn make_json_from_file(file: PathBuf) {
    let file_content = std::fs::read_to_string(file).unwrap();
    let json_content = serde_json::to_string(&file_content).unwrap();

    println!("{}", json_content);
}
