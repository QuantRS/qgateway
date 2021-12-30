
mod yaml;
mod config;
mod server;

#[tokio::main]
async fn main() {
    let conf = &yaml::load_file("config.yaml")[0];
    let auth_tokens = conf["auth-token"].as_vec().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect::<Vec<String>>();
    let queues = conf["queues"].as_vec().unwrap().iter().map(|x| config::Queue::new(x)).collect::<Vec<config::Queue>>();

    let mut server = server::Server::new(auth_tokens, queues);
    server.start(conf["address"].as_str().unwrap()).await;
}
