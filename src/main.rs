use wasmer::{Store, Module, Instance, Value, imports};

use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_tungstenite::{connect_async, 
    tungstenite::protocol::Message,
tungstenite::{Error, Result},
};
use url::Url;

#[tokio::main]
async fn main() {
    let url = Url::parse("ws://127.0.0.1:3000/ws").unwrap();
    let (mut ws_stream, _) = connect_async(url).await.expect("Failed to connect");
    println!("WebSocket handshake has been successfully completed");
    
    while let Some(msg) = ws_stream.next().await {
        let data = msg.unwrap().into_data();

        let store = Store::default();
        let module = Module::new(&store, &data).unwrap();
        // The module doesn't import anything, so we create an empty import object.
        let import_object = imports! {};
        let instance = Instance::new(&module, &import_object).unwrap();

        let worker_func = instance.exports.get_function("run").unwrap();
        let result = worker_func.call(&[Value::I32(0)]).unwrap();
        ws_stream.send(Message::text(result.to_vec()[0].to_string())).await.expect("send failed");
    }
}
