use futures::Future;
use serde::Serialize;
use sessions::{Client, Clients, Sessions};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use warp::filters::BoxedFilter;
use warp::ws::Message;
use warp::{Filter, Reply};

/// Definitions for Client and Session Scheme with some helper methods
pub mod sessions;

mod handler;
mod ws;

/// Wrapper for an object that is safe across threads and has a long lifespan in order to satisfy warp filters + async
type SafeResource<T> = Arc<RwLock<T>>;

/// An Async safe Client Storage with runtime controlled cleanup (by Arc<RwLock<T>>)
pub type SafeClients = SafeResource<Clients>;
/// An Async safe Session Storage with runtime controlled cleanup (by Arc<RwLock<T>>)
pub type SafeSessions<T> = SafeResource<Sessions<T>>;

/// Configuration for the server,
/// including variables, callbacks for the server
pub struct ServerConfig<T, Fut1, Fut2>
where
    Fut1: Future<Output = ()>,
    Fut2: Future<Output = ()>,
{
    /// An async function pointer to a handler for the server tick
    pub tick_handler: Option<fn(SafeClients, SafeSessions<T>) -> Fut1>,
    /// An async function pointer to a handler for websocket message events
    ///
    /// The events will be parsed to strings before getting passed to the handler
    pub event_handler: fn(String, String, SafeClients, SafeSessions<T>) -> Fut2,
}

/// Composite backend and frontend routes for the entire server
///
/// ```
/// use websocket_server::*;
///
/// async fn custom_tick_handler(_: SafeClients, _: SafeSessions<()>) {}
/// async fn custom_message_handler(_: String, _: String, _: SafeClients, _: SafeSessions<()>) {}
///
/// #[tokio::main]
/// async fn main() {
///     let app = server(ServerConfig {
///         event_handler: custom_message_handler,
///         tick_handler: Some(custom_tick_handler),
///     });
/// }
/// ```
pub fn server<T, Fut1, Fut2>(
    server_config: ServerConfig<T, Fut1, Fut2>,
) -> BoxedFilter<(impl Reply,)>
where
    T: 'static + Clone + Send + Sync,
    Fut1: 'static + Future<Output = ()> + Send + Sync,
    Fut2: 'static + Future<Output = ()> + Send + Sync,
{
    // Routes handling server requests and connections
    let backend = {
        let clients: SafeClients = Arc::new(RwLock::new(HashMap::new()));
        let sessions: SafeSessions<T> = Arc::new(RwLock::new(HashMap::new()));
        let health = warp::path!("health").and_then(handler::health_handler);

        let (clients1, sessions1) = (clients.clone(), sessions.clone());
        let socket = warp::path("ws")
            .and(warp::ws())
            .and(warp::path::param())
            // pass copies of our references for the client and sessions maps to our handler
            .and(warp::any().map(move || clients1.clone()))
            .and(warp::any().map(move || sessions1.clone()))
            .and(warp::any().map(move || server_config.event_handler))
            .and_then(handler::ws_handler);

        match server_config.tick_handler {
            Some(tick_handler) => {
                tokio::spawn(async move {
                    log::info!("starting server tick task..");
                    loop {
                        tick_handler(clients.clone(), sessions.clone()).await;
                    }
                });
            }
            None => log::info!("server was not set to not run a tick handler."),
        }

        health.or(socket).boxed()
    };

    // Routes for serving static website files
    let frontend = warp::fs::dir("dist").boxed();

    warp::path("api").and(backend).or(frontend).boxed()
}

/// Send a websocket message to a single client
pub fn message_client<T>(client: &Client, message: &T)
where
    T: Serialize,
{
    let sender = match &client.sender {
        Some(s) => s,
        None => {
            return log::error!(
                "sender was lost for clienT: Send + Sync + 'static {}",
                client.id
            )
        }
    };
    if let Err(e) = sender.send(Ok(Message::text(serde_json::to_string(message).unwrap()))) {
        log::error!("failed to send message to {} with err: {}", client.id, e,);
    }
}

/// Remove a sessions and the possible game state that accompanies it
pub fn cleanup_session<T>(session_id: &str, sessions: &mut Sessions<T>) {
    // remove session
    sessions.remove(session_id);
    // log status
    log::info!("removed empty session");
    log::info!("sessions live: {}", sessions.len());
}
