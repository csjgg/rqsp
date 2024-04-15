use crate::jsonfile::read_json_file;

pub struct Client {
    bind: String,
    target: String,
}

impl Client {
    pub async fn init() -> Client {
        let json = read_json_file("config.json").await;
        if json["client"]["bind"].is_null() {
            panic!("Client bind ip and port are not defined in config.json");
        }
        if json["client"]["target"].as_str().unwrap().is_empty() {
            panic!("Target ip and port are not defined in config.json");
        }
        Client {
            bind: json["client"]["bind"].as_str().unwrap().to_string(),
            target: json["client"]["target"].as_str().unwrap().to_string(),
        }
    }
    pub fn get_bind(&self) -> String {
        self.bind.clone()
    }
    pub fn get_target(&self) -> String {
        self.target.clone()
    }
}
