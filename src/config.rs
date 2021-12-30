use yaml_rust::Yaml;


#[derive(Debug)]
pub struct Queue {
    pub token: String,
    pub queue_type: String,
    pub send_white: bool,
    pub send_token: Vec<String>,
    pub recv_white: bool,
    pub recv_token: Vec<String>
}

impl Queue {
    pub fn new(yaml: &Yaml) -> Queue {
        return Queue{
            token: yaml["token"].as_str().unwrap().to_string(),
            queue_type: yaml["type"].as_str().unwrap().to_string(),
            send_white: yaml["send_white"].as_bool().unwrap(),
            send_token: yaml["send_token"].as_vec().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect::<Vec<String>>(),
            recv_white: yaml["recv_white"].as_bool().unwrap(),
            recv_token: yaml["recv_token"].as_vec().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect::<Vec<String>>(),
        }
    }
}