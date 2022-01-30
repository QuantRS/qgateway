use std::{collections::HashMap, net::SocketAddr, sync::{Arc, Mutex}};

use futures::{StreamExt, TryStreamExt, channel::mpsc::{self, UnboundedSender}, future, pin_mut};


use protobuf::Message;
use tokio::net::{TcpListener};

use chrono::{Local, Timelike};
use tokio_tungstenite::tungstenite::Message as WSMessage;

use crate::{config, protocol};

pub struct Server {
    auth_tokens: Vec<String>,
    auth_status: Arc<Mutex<HashMap<SocketAddr, (String, bool)>>>,
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
            clients: HashMap::new(),
            send_white: config_queue.send_white,
            send_token: config_queue.send_token,
            recv_white: config_queue.recv_white,
            recv_token: config_queue.recv_token,

            second: -1,
            second_count: 0,
        });
    }

    pub async fn start(&mut self, address: &str, debug: bool) {
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
                    let auth_status_clone = auth_status_clone.clone();
                    let mut auth_status = auth_status_clone.lock().unwrap();

                    let queues_clone = queues_clone.clone();
                    let mut queues = queues_clone.lock().unwrap();

                    if debug {
                        println!("Received a message from {}: {}", addr, msg.to_text().unwrap());
                    }
                    
                    match msg {
                        WSMessage::Binary(msg) => {
                            let req = protocol::Request::parse_from_bytes(&msg);
                            if req.is_err() {
                                println!("{:?}", req.err().unwrap());
    
                                tx.unbounded_send(WSMessage::Close(None)).unwrap();
                                return future::ok(());
                            }

                            let req = req.unwrap();
                            match req.get_command() {
                                protocol::Commands::HEARTBEAT => {
                                    //心跳包
                                },
                                protocol::Commands::LOGIN => {
                                    let mut login_req = protocol::LoginRequest::parse_from_bytes(req.get_data()).unwrap();

                                    // 校验 并登录
                                    if !login_req.get_token().eq("") && !auth_tokens_clone.contains(&login_req.mut_token()) {
                                        tx.unbounded_send(WSMessage::Close(None)).unwrap();
                                        println!("{}: token is error", addr);
                                        return future::ok(());
                                    }
                                    auth_status.insert(addr, (login_req.get_token().to_string(), true));

                                    let mut res = protocol::Response::new();
                                    res.set_command(protocol::Commands::LOGIN);

                                    let mut login_res = protocol::LoginResponse::new();
                                    login_res.set_status(true);

                                    res.set_data(login_res.write_to_bytes().unwrap());
                                    res.set_message("login success.".to_string());

                                    tx.unbounded_send(WSMessage::Binary(res.write_to_bytes().unwrap())).unwrap();
                                },
                                protocol::Commands::SEND_MESSAGE => {
                                    if !auth_status.contains_key(&addr) || !auth_status.get(&addr).unwrap().1 {
                                        println!("{}: not login.", addr);
                                        return future::ok(());
                                    }

                                    let mut send_req = protocol::SendRequest::parse_from_bytes(req.get_data()).unwrap();

                                    let queue = queues.get_mut(send_req.get_token()).unwrap();
                                    if queue.send_white && !queue.send_token.contains(&auth_status.get(&addr).unwrap().0) {
                                        tx.unbounded_send(WSMessage::Close(None)).unwrap();
                                        println!("{}: {} queue not in send white list", addr, send_req.get_token());
                                        return future::ok(());
                                    }

                                    //计算 /s
                                    let second = Local::now().second() as i32;
                                    if queue.second == second {
                                        queue.second_count += 1;
                                    } else {
                                        queue.second = second;
                                        queue.second_count = 1;
                                    }

                                    if queue.queue_type.eq("router") {
                                        for (_, client) in &queue.clients {
                                            if client.keys.contains(&send_req.mut_key()) {
                                                let mut res = protocol::Response::new();
                                                res.set_command(protocol::Commands::SUBSCRIBE_CALLBACK);
            
                                                let mut callback = protocol::SubscribeCallback::new();
                                                callback.set_token(send_req.get_token().to_string());
                                                callback.set_key(send_req.get_key().to_string());
                                                callback.set_data(send_req.get_data().to_vec());
                                                
                                                res.set_data(callback.write_to_bytes().unwrap());

                                                client.tx.unbounded_send(WSMessage::Binary(res.write_to_bytes().unwrap())).unwrap();
                                            }
                                        }
                                    } else if  queue.queue_type.eq("pubsub") {
                                        for (_, client) in &queue.clients {
                                            let mut res = protocol::Response::new();
                                            res.set_command(protocol::Commands::SUBSCRIBE_CALLBACK);
        
                                            let mut callback = protocol::SubscribeCallback::new();
                                            callback.set_token(send_req.get_token().to_string());
                                            callback.set_data(send_req.get_data().to_vec());
                                            
                                            res.set_data(callback.write_to_bytes().unwrap());

                                            client.tx.unbounded_send(WSMessage::Binary(res.write_to_bytes().unwrap())).unwrap();
                                        }
                                    }
                                },
                                protocol::Commands::SUBSCRIBE => {
                                    if !auth_status.contains_key(&addr) || !auth_status.get(&addr).unwrap().1 {
                                        println!("{}: not login.", addr);
                                        return future::ok(());
                                    }

                                    let mut subscribe_req = protocol::SubscribeRequest::parse_from_bytes(req.get_data()).unwrap();

                                    let queue = queues.get_mut(subscribe_req.get_token()).unwrap();
                                    if queue.recv_white && !queue.recv_token.contains(&auth_status.get(&addr).unwrap().0) {
                                        tx.unbounded_send(WSMessage::Close(None)).unwrap();
                                        println!("{}: {} queue not in recv white list", addr, subscribe_req.get_token());
                                        return future::ok(());
                                    }

                                    queue.clients.insert(addr, SubClient{
                                        keys: subscribe_req.mut_keys().to_vec(),
                                        tx: tx.clone(),
                                    });

                                    let mut res = protocol::Response::new();
                                    res.set_command(protocol::Commands::SUBSCRIBE);

                                    let mut subscribe_res = protocol::SubscribeResponse::new();
                                    subscribe_res.set_token(subscribe_req.get_token().to_string());
                                    subscribe_res.set_success(true);
 
                                    res.set_data(subscribe_res.write_to_bytes().unwrap());

                                    tx.unbounded_send(WSMessage::Binary(res.write_to_bytes().unwrap())).unwrap();
                                },
                                /* protocol::Commands::STATUS => {
                                    if !auth_status.contains_key(&addr) || !auth_status.get(&addr).unwrap().1 {
                                        println!("{}: not login.", addr);
                                        return future::ok(());
                                    }

                                    let args = req.args.as_object().unwrap();
                                    let token = args["token"].as_str().unwrap();

                                    let queue = queues.get_mut(token);
                                    if queue.is_none() {
                                        let res = Response{
                                            cmd_id: 3,
                                            data: Value::Null,
                                            message: Value::String("error.".to_string())
                                        };
                                        tx.unbounded_send(Message::Text(serde_json::to_string(&res).unwrap())).unwrap();
                                        return future::ok(());
                                    }

                                    let res = Response{
                                        cmd_id: 3,
                                        data: Value::String(token.to_string()),
                                        message: Value::String(format!("message speed: {}/s", queue.unwrap().second_count))
                                    };
                                    tx.unbounded_send(Message::Text(serde_json::to_string(&res).unwrap())).unwrap();
                                }, */
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
    tx: UnboundedSender<WSMessage>
}

struct Queue {
    queue_type: String,
    send_white: bool,
    send_token: Vec<String>,
    recv_white: bool,
    recv_token: Vec<String>,
    clients: HashMap<SocketAddr, SubClient>,

    second: i32,
    second_count: i32,
}
