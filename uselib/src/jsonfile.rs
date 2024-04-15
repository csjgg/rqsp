use json::JsonValue;
use tokio::fs::File;
use tokio::io::AsyncReadExt;

pub async fn read_json_file(file_path: &str) ->JsonValue{
  let mut file = File::open(file_path).await.unwrap();
  let mut contents = String::new();
  file.read_to_string(&mut contents).await.unwrap();
  json::parse(&contents).unwrap()
}
