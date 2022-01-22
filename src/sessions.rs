use std::collections::HashMap;
use tokio::sync::mpsc;
use warp::ws::Message;

/// A Server-Side Container for Clients
pub type Clients = HashMap<String, Client>;
/// A Server-Side Container for Sessions
pub type Sessions<T> = HashMap<String, Session<T>>;

/// Data Stored for a Single User in a Session
#[derive(Debug, Clone)]
pub struct Client {
    /// Unique ID of the Client
    pub id: String,
    /// Session ID the client belongs to if it exists
    pub session_id: Option<String>,
    /// Sender pipe used to relay messages to the sender socket
    pub sender: Option<mpsc::UnboundedSender<std::result::Result<Message, warp::Error>>>,
}

/// Data Stored for a Session
#[derive(Debug, Clone)]
pub struct Session<T> {
    /// Unique ID of the Session
    pub id: String,
    /// ID of the Client who owns the Session
    pub owner: String,
    /// Active statuses of every Client in the Session
    ///
    /// This is intended to be used to keep a Session alive until it is forcefully closed or all of its Clients have disconnected
    pub client_statuses: HashMap<String, bool>,
    /// Data that you want to store in a sessions
    pub data: T,
}

impl<T> Session<T> {
    /// get the number of clients in the session
    pub fn get_num_clients(&self) -> usize {
        self.client_statuses.len()
    }

    /// check if a client_id exists in the session
    pub fn contains_client(&self, id: &str) -> bool {
        self.client_statuses.contains_key(id)
    }

    /// fetch a Vec of clients in the Session
    pub fn get_client_ids(&self) -> Vec<&String> {
        self.client_statuses.iter().map(|(id, _)| id).collect()
    }

    /// remove a client from the Session using their ID
    pub fn remove_client(&mut self, id: &str) {
        self.client_statuses.remove(id);
    }

    /// add a new Client into the Session and mark them as active
    pub fn insert_client(&mut self, id: &str, is_active: bool) {
        self.client_statuses.insert(id.to_string(), is_active);
    }

    /// fetch a Vec of all Clients in the Session that are currently active
    pub fn get_clients_with_active_status(&self, active_status: bool) -> Vec<&String> {
        self.client_statuses
            .iter()
            .filter(|&(_, status)| *status == active_status)
            .map(|(id, _)| id)
            .collect()
    }

    /// Set the active status of a Client within the session
    ///
    /// This is ideally performed whenever a client disconnects or reconnects to a Session that is still running
    pub fn set_client_active_status(&mut self, id: &str, is_active: bool) -> Result<(), String> {
        match self.client_statuses.get(id) {
            Some(_) => {
                self.client_statuses.insert(id.to_string(), is_active);
                Ok(())
            }
            None => Err(format!(
                "tried to set active_status of client: {} but id was not found in session",
                id
            )),
        }
    }
}
