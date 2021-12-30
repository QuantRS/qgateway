use std::{collections::HashMap, net::SocketAddr, sync::{Arc, Mutex}};

use futures::{StreamExt, TryStreamExt, channel::mpsc, future, pin_mut};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::net::{TcpListener};
use tokio_tungstenite::tungstenite::Message;

pub struct Server;

impl Server {
    pub fn new() -> Server {
        return Server;
    }

    pub async fn start(&mut self, address: &str) {
        let try_socket = TcpListener::bind(address).await;
        let listener = try_socket.expect("Failed to bind");
        println!("Listening on: ws://{}", address);
    
        while let Ok((stream, addr)) = listener.accept().await {
            tokio::spawn(async move {
                println!("Incoming TCP connection from: {}", addr);
    
                let ws_stream = tokio_tungstenite::accept_async(stream)
                    .await
                    .expect("Error during the websocket handshake occurred");
                println!("WebSocket connection established: {}", addr);

                let (tx, rx) = mpsc::unbounded();

                let (outgoing, incoming) = ws_stream.split();
            
                let broadcast_incoming = incoming.try_for_each(|msg| {
                    println!("Received a message from {}: {}", addr, msg.to_text().unwrap());

                    match msg {
                        Message::Text(msg) => {
                            let json = serde_json::from_str(&msg.to_string());
                            if !json.is_ok() {
                                println!("{:?}", json.err().unwrap());
    
                                tx.unbounded_send(Message::Close(None)).unwrap();
                                return future::ok(());
                            }
    
                            let req: Request = json.unwrap();
                        }
                        _ => {}
                    }

                    future::ok(())
                });

                let receive_from_others = rx.map(Ok).forward(outgoing);

                pin_mut!(broadcast_incoming, receive_from_others);
                future::select(broadcast_incoming, receive_from_others).await;

                println!("{} disconnected", &addr);
                sub_data_clone.lock().unwrap().remove(&addr);
                sub_tick_clone.lock().unwrap().remove(&addr);
            });
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct Request {
    #[serde(rename = "cmdId")]
    cmd_id: i32,
    #[serde(default = "Vec::new")]
    args: Vec<String>
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

