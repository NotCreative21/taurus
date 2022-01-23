use crate::Clients;
use warp::ws::Message;

pub async fn send_chat(clients: &Clients, msg: String) {
    let locked = clients.lock().await;
    for (key, _) in locked.iter() {
        match locked.get(key) {
            Some(t) => {
                if let Some(t) = &t.sender {
                    let _ = t.send(Ok(Message::text("CHAT_OUT ".to_owned() + &msg)));
                }
            }
            None => continue,
        };
    }
}
