use crate::jsonfile::read_json_file;

pub struct Server {
    bind: String,
}

impl Server {
    pub async fn init() -> Server {
        let json = read_json_file("config.json").await;
        if json["server"]["bind"].is_null() {
            panic!("clinet bind ip and port are not defined in config.json");
        }
        Server {
            bind: json["server"]["bind"].as_str().unwrap().to_string(),
        }
    }
    pub fn get_bind(&self) -> String {
        self.bind.clone()
    }
}
