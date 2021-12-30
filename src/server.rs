use std::{collections::HashMap, net::SocketAddr, sync::{Arc, Mutex}};

use futures::{StreamExt, TryStreamExt, channel::mpsc::{self, UnboundedSender}, future, pin_mut};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::net::{TcpListener};
use tokio_tungstenite::tungstenite::Message;

use crate::config;

pub struct Server {
    auth_tokens: Vec<String>,
    auth_status: Arc<Mutex<HashMap<SocketAddr, bool>>>,
    queues: Arc<Mutex<HashMap<String, Queue>>>,
}

impl Server {
    pub fn new(auth_tokens: Vec<String>, queues: Vec<config::Queue>) -> Server {
        let mut server = Server{
            auth_tokens,
            auth_status: Arc::new(Mutex::new(HashMap::new())),
            queues: Arc::new(Mutex::new(HashMap::new())),
        };

        for queue in queues {
            server.create_queue(queue)
        }

        return server;
    }

    fn create_queue(&mut self, config_queue: config::Queue) {
        self.queues.lock().unwrap().insert(config_queue.token, Queue{
            queue_type: config_queue.queue_type,
            clients: HashMap::new()
        });
    }

    pub async fn start(&mut self, address: &str) {
        let try_socket = TcpListener::bind(address).await;
        let listener = try_socket.expect("Failed to bind");
        println!("Listening on: ws://{}", address);
    
        while let Ok((stream, addr)) = listener.accept().await {
            let auth_tokens_clone = self.auth_tokens.clone();
            let auth_status_clone = self.auth_status.clone();
            let queues_clone = self.queues.clone();

            tokio::spawn(async move {
                println!("Incoming TCP connection from: {}", addr);
    
                let ws_stream = tokio_tungstenite::accept_async(stream)
                    .await
                    .expect("Error during the websocket handshake occurred");
                println!("WebSocket connection established: {}", addr);

                let (tx, rx) = mpsc::unbounded();

                let (outgoing, incoming) = ws_stream.split();
            
                let broadcast_incoming = incoming.try_for_each(|msg| {
                    //println!("Received a message from {}: {}", addr, msg.to_text().unwrap());

                    match msg {
                        Message::Text(msg) => {
                            let json = serde_json::from_str(&msg.to_string());
                            if !json.is_ok() {
                                println!("{:?}", json.err().unwrap());
    
                                tx.unbounded_send(Message::Close(None)).unwrap();
                                return future::ok(());
                            }
    
                            let req: Request = json.unwrap();
                            match req.cmd_id {
                                -1 => {
                                    if !auth_status_clone.lock().unwrap().contains_key(&addr) {
                                        return future::ok(());
                                    }

                                    let res = Response{
                                        cmd_id: -1,
                                        data: Value::Null,
                                        message: Value::Null
                                    };
                                    tx.unbounded_send(Message::Text(serde_json::to_string(&res).unwrap())).unwrap();
                                },
                                0 => {
                                    let mut token = "".to_string();
                                    if req.args.is_array() {
                                        token = req.args.as_array().unwrap()[0].as_str().unwrap().to_string();
                                    } else if req.args.is_object() {
                                        token = req.args.as_object().unwrap()["token"].as_str().unwrap().to_string();
                                    }

                                    if token.eq("") {
                                        tx.unbounded_send(Message::Close(None)).unwrap();
                                        println!("{}: token is null", addr);
                                        return future::ok(());
                                    }

                                    if !auth_tokens_clone.contains(&token) {
                                        tx.unbounded_send(Message::Close(None)).unwrap();
                                        println!("{}: token is error", addr);
                                        return future::ok(());
                                    }

                                    auth_status_clone.lock().unwrap().insert(addr, true);
                                    let res = Response{
                                        cmd_id: 0,
                                        data: Value::Null,
                                        message: Value::String("login success.".to_string())
                                    };
                                    tx.unbounded_send(Message::Text(serde_json::to_string(&res).unwrap())).unwrap();
                                },
                                1 => {
                                    let args = req.args.as_object().unwrap();
                                    let token = args["token"].as_str().unwrap();
                                    let value = args["value"].to_owned();

                                    let mut queue_value = serde_json::Map::new();
                                    queue_value.insert("token".to_string(), Value::String(token.to_string()));
                                    queue_value.insert("value".to_string(), value);

                                    let mut queue = queues_clone.lock().unwrap();
                                    let queue = queue.get_mut(token).unwrap();

                                    if queue.queue_type.eq("router") {
                                        let key = args["key"].as_str().unwrap().to_string();
                                        for (_, client) in &queue.clients {
                                            if client.keys.contains(&key) {
                                                let res = Response{
                                                    cmd_id: 2,
                                                    data: Value::Object(queue_value.clone()),
                                                    message: Value::Null
                                                };
                                                client.tx.unbounded_send(Message::Text(serde_json::to_string(&res).unwrap())).unwrap();
                                            }
                                        }
                                    } else if  queue.queue_type.eq("pubsub") {
                                        for (_, client) in &queue.clients {
                                            let res = Response{
                                                cmd_id: 2,
                                                data: Value::Object(queue_value.clone()),
                                                message: Value::Null
                                            };
                                            client.tx.unbounded_send(Message::Text(serde_json::to_string(&res).unwrap())).unwrap();
                                        }
                                    }
                                },
                                2 => {
                                    let args = req.args.as_object().unwrap();
                                    let token = args["token"].as_str().unwrap();

                                    let mut queue = queues_clone.lock().unwrap();
                                    let queue = queue.get_mut(token).unwrap();

                                    let mut keys = Vec::new();
                                    if queue.queue_type.eq("router") {
                                        if args.contains_key("key") {
                                            keys.push(args["key"].as_str().unwrap().to_string())
                                        } else if args.contains_key("keys") {
                                            keys.extend(args["keys"].as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect::<Vec<String>>());
                                        }
                                    }

                                    queue.clients.insert(addr, SubClient{
                                        keys,
                                        tx: tx.clone(),
                                    });

                                    let res = Response{
                                        cmd_id: 0,
                                        data: Value::Null,
                                        message: Value::String("subscribe success.".to_string())
                                    };
                                    tx.unbounded_send(Message::Text(serde_json::to_string(&res).unwrap())).unwrap();
                                },
                                _ => {}
                            }
                        }
                        _ => {}
                    }

                    future::ok(())
                });

                let receive_from_others = rx.map(Ok).forward(outgoing);

                pin_mut!(broadcast_incoming, receive_from_others);
                future::select(broadcast_incoming, receive_from_others).await;

                println!("{} disconnected", &addr);
                auth_status_clone.lock().unwrap().remove(&addr);
                
                for (_, queue) in queues_clone.lock().unwrap().iter_mut() {
                    queue.clients.remove(&addr);
                }
            });
        }
    }
}

struct SubClient {
    keys: Vec<String>,
    tx: UnboundedSender<Message>
}

struct Queue {
    queue_type: String,
    clients: HashMap<SocketAddr, SubClient>
}

#[derive(Debug, Clone, Deserialize)]
struct Request {
    #[serde(rename = "cmdId")]
    cmd_id: i32,
    args: Value
}


#[derive(Debug, Clone, Serialize)]
struct Response {
    #[serde(rename = "cmdId")]
    cmd_id: i32,
    #[serde(skip_serializing_if = "Value::is_null")]
    data: Value,
    #[serde(skip_serializing_if = "Value::is_null")]
    message: Value
}

