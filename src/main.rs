use std::{collections::HashMap, borrow::Borrow};

use wasmer::{Store, Module, Instance, Value, imports, CompileError};

use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_tungstenite::{connect_async, 
    tungstenite::protocol::Message,
tungstenite::{Error, Result},
};
use reqwest;
use url::Url;

use minion_msg::{self, MinionErrors};
use minion_msg::{MinionOps, MinionMsg, MinionId};

struct MemoisedModules {
    store: HashMap<String, Option<Module>>
}


impl MemoisedModules {
    fn new() -> Self {
        MemoisedModules{
            store: HashMap::new()
        }
    }

    async fn get(&mut self, uri: &str) -> Option<Module> {
       if let Some(m) = self.store.get(uri) {
           return m.clone();
       } 

       let url = "http://127.0.0.1:3000".to_owned() + &uri.to_owned();
       println!("{url}");
        if let Ok(resp) = reqwest::get(url).await {
            println!("{:?}", resp);
            if let Ok(bytes) = resp.text().await {
                let store = Store::default();
                let module = Module::new(&store, bytes);
                if let Ok(m) = module {
                    self.store.insert(uri.to_owned(), Some(m.clone()));
                    return Some(m);
                }
            }
        }
        None
    }
}


#[tokio::main]
async fn main() {
    let mut mod_cache = MemoisedModules::new();

    let url = Url::parse("ws://127.0.0.1:3000/ws").unwrap();
    let (mut ws_stream, _) = connect_async(url).await.expect("Failed to connect");
    println!("WebSocket handshake has been successfully completed");
    
    let msg = minion_msg::to_vec(&minion_msg::MinionMsg{ 
        op: MinionOps::Auth,
        payload: MinionId::new().into(),
    }).unwrap();
    ws_stream.send(Message::Binary(msg)).await.expect("send failed");


    while let Some(msg) = ws_stream.next().await { 
        match msg {
            Ok(msg) => match msg {
                Message::Text(t) => {
                    println!("why did server send str: {:?}", t);
                }
                Message::Binary(b) => {
                    println!("server sent binary data");
                    let m = minion_msg::from_bytes(&b).unwrap();
                    println!("op: {:?}", m.op);
                    match m.op { 
                        MinionOps::Exec => {
                            let uri: &str = &String::from_utf8_lossy(&m.payload);
                            let module = match mod_cache.get(uri).await {
                                Some(m) => m,
                                None => {
                                    ws_stream.send(
                                        Message::Binary(
                                            minion_msg::to_vec(
                                                &MinionMsg{
                                                    op: MinionOps::Error,
                                                    payload: MinionErrors::BadCode.into()
                                                }).unwrap()
                                            )).await.expect("send failed");
                                    continue;
                                }
                            };
                            
                            // The module doesn't import anything, so we create an empty import object.
                            let import_object = imports! {};
                            let instance = Instance::new(&module, &import_object).unwrap();

                            let worker_func = instance.exports.get_function("run").unwrap();
                            let result = worker_func.call(&[]).unwrap();

                            ws_stream.send(
                                Message::Binary(
                                    minion_msg::to_vec(
                                        &MinionMsg{
                                        op: MinionOps::Ret,
                                        payload: Vec::from_iter(
                                            result.to_vec().
                                            iter().map(
                                                |x| x.i32().unwrap().to_ne_bytes()
                                            ).flatten()
                                        ), 
                                    }).unwrap())
                            ).await.expect("send failed");
                        }
                        _ => {
                            println!("server bad {:?}", m.op);
                        }
                    }
                }
                Message::Ping(_) => {
                    println!("socket ping");
                }
                Message::Pong(_) => {
                    println!("socket pong");
                }
                Message::Close(_) => {
                    println!("client disconnected");
                    return;
                }
                _ => {
                    println!("shouldn't happen but who knew");
                }
            }
            Err(e) => {
                println!("error {:?}", e);
                return;
            }
        }
    }
}
