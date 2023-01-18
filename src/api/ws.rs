use std::{net::SocketAddr, ops::ControlFlow, sync::Arc};

use axum::{
    extract::{
        ws::{Message, WebSocket},
        ConnectInfo, WebSocketUpgrade,
    },
    headers::UserAgent,
    response::IntoResponse,
    Extension, TypedHeader,
};
use futures::StreamExt;
use hyper::StatusCode;
use log::{debug, info};

use crate::handle_unauthorized;

use super::{KndMacaroon, MacaroonAuth};

/// This is WIP. Just connects and checks macaroon at the moment.

/// The handler for the HTTP request (this gets called when the HTTP GET lands at the start
/// of websocket negotiation. After this completes, the actual switching from HTTP to
/// websocket protocol will occur.
/// This is the last point where we can extract TCP/IP metadata such as IP address of the client
/// as well as things from HTTP headers such as user-agent of the browser etc.
pub async fn ws_handler(
    macaroon: KndMacaroon,
    Extension(macaroon_auth): Extension<Arc<MacaroonAuth>>,
    ws: WebSocketUpgrade,
    user_agent: Option<TypedHeader<UserAgent>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> Result<impl IntoResponse, StatusCode> {
    handle_unauthorized!(macaroon_auth.verify_admin_macaroon(&macaroon.0));
    let user_agent = user_agent
        .map(|a| a.to_string())
        .unwrap_or_else(|| "Unknown client".to_string());

    info!("`{}` at {} connected.", user_agent, addr.to_string());
    // finalize the upgrade process by returning upgrade callback.
    // we can customize the callback by sending additional info such as address.
    Ok(ws
        .protocols(["hex"])
        .on_upgrade(move |socket| handle_socket(socket, addr)))
}

/// Actual websocket statemachine (one will be spawned per connection)
async fn handle_socket(mut socket: WebSocket, who: SocketAddr) {
    //send a ping (unsupported by some browsers) just to kick things off and get a response
    if socket.send(Message::Ping(vec![])).await.is_ok() {
        debug!("Pinged {}...", who);
    } else {
        debug!("Could not send ping {}!", who);
        // no Error here since the only thing we can do is to close the connection.
        // If we can not send messages, there is no way to salvage the statemachine anyway.
        return;
    }

    // Since each client gets individual statemachine, we can pause handling
    // when necessary to wait for some external event (in this case illustrated by sleeping).
    // Waiting for this client to finish getting his greetings does not prevent other clients form
    // connecting to server and receiving their greetings.

    // By splitting socket we can send and receive at the same time. In this example we will send
    // unsolicited messages to client based on some sort of server's internal event (i.e .timer).
    let (mut _sender, mut receiver) = socket.split();

    /* To close connection.
        debug!("Sending close to {}...", who);
        if let Err(e) = sender
            .send(Message::Close(Some(CloseFrame {
                code: axum::extract::ws::close_code::NORMAL,
                reason: Cow::from("Goodbye"),
            })))
            .await
        {
            debug!("Could not send Close due to {}, probably it is ok?", e);
        }
    });*/

    // This second task will receive messages from client and print them on server console
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            // print message and break if instructed to do so
            if process_message(msg, who).is_break() {
                break;
            }
        }
    });

    recv_task.await.unwrap();

    // returning from the handler closes the websocket connection
    info!("Websocket context {} destroyed", who);
}

/// helper to print contents of messages to stdout. Has special treatment for Close.
fn process_message(msg: Message, who: SocketAddr) -> ControlFlow<(), ()> {
    match msg {
        Message::Text(t) => {
            info!(">>> {} sent str: {:?}", who, t);
        }
        Message::Binary(d) => {
            info!(">>> {} sent {} bytes: {:?}", who, d.len(), d);
        }
        Message::Close(c) => {
            if let Some(cf) = c {
                info!(
                    ">>> {} sent close with code {} and reason `{}`",
                    who, cf.code, cf.reason
                );
            } else {
                info!(">>> {} somehow sent close message without CloseFrame", who);
            }
            return ControlFlow::Break(());
        }

        Message::Pong(v) => {
            info!(">>> {} sent pong with {:?}", who, v);
        }
        // You should never need to manually handle Message::Ping, as axum's websocket library
        // will do so for you automagically by replying with Pong and copying the v according to
        // spec. But if you need the contents of the pings you can see them here.
        Message::Ping(v) => {
            info!(">>> {} sent ping with {:?}", who, v);
        }
    }
    ControlFlow::Continue(())
}
