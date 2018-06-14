extern crate ws;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate serde;

mod game;

use std::thread;
use std::time::Duration;
use std::sync::{Mutex, Arc};
use ws::{connect, Handler, Sender, Handshake, Message, CloseCode};
use serde::{Deserialize, Serialize, Deserializer, Serializer};

#[derive(Debug,Serialize)]
struct NewLobbyMessage {
    max_players: u8,
    password: String,
    lobby_name: String,
    player_name: String,
}

#[derive(Debug,Deserialize)]
struct NewLobbyResponse {
    player_id: String,
    lobby_id: String,
}

#[derive(Deserialize, Debug)]
enum NewLobbyError {
    LessThanTwoMaxPlayers,
    EmptyLobbyName,
    EmptyPlayerName,
}

#[derive(Debug,Serialize)]
struct JoinLobbyMessage {
    lobby_id: String,
    player_name: String,
    password: String,
}

#[derive(Debug,Deserialize)]
struct JoinLobbyResponse {
    player_id: String,
}

#[derive(Deserialize, Debug)]
enum JoinLobbyError {
    LobbyNotFound,
    LobbyFull,
    BadPassword,
    GameStarted,
}

#[derive(Debug, Serialize)]
struct StartGameMessage {
    lobby_id: String,
    player_id: String,
}

#[derive(Debug, Serialize)]
struct ChooseFaceupMessage {
    lobby_id: String,
    player_id: String,
    card_one: game::Card,
    card_two: game::Card,
    card_three: game::Card,
}

#[derive(Debug,Serialize)]
enum PalaceMessage {
    NewLobby(NewLobbyMessage),
    JoinLobby(JoinLobbyMessage),
    ListLobbies,
    StartGame(StartGameMessage),
    ChooseFaceup(ChooseFaceupMessage),
}

#[derive(Debug,Deserialize)]
enum PalaceOutMessage {
    NewLobbyResponse(Result<NewLobbyResponse, NewLobbyError>),
    JoinLobbyResponse(Result<JoinLobbyResponse, JoinLobbyError>),
    LobbyList(Box<[LobbyDisplay]>),
    PublicGameState(game::PublicGameState),
    Hand(Box<[game::Card]>),
}

#[derive(Debug,Serialize, Deserialize)]
struct LobbyDisplay {
    cur_players: u8,
    max_players: u8,
    started: bool,
    has_password: bool,
    owner: String,
    name: String,
    age: u64,
}

#[derive(Clone)]
enum MutexStatus {
    Unmodified,
    InProgress,
    Finished(String)
}


// Our Handler struct.
// Here we explicity indicate that the Client needs a Sender,
// whereas a closure captures the Sender for us automatically.
struct Client {
    out: Sender,
    player_id: Option<String>,
    player_index: Option<String>,
    lobby_id_mutex: Arc<Mutex<MutexStatus>>,
    lobby_id: Option<String>,
}

// We implement the Handler trait for Client so that we can get more
// fine-grained control of the connection.
impl Handler for Client {

    // `on_open` will be called only after the WebSocket handshake is successful
    // so at this point we know that the connection is ready to send/receive messages.
    // We ignore the `Handshake` for now, but you could also use this method to setup
    // Handler state or reject the connection based on the details of the Request
    // or Response, such as by checking cookies or Auth headers.
    fn on_open(&mut self, _: Handshake) -> ws::Result<()> {
        // Now we don't need to call unwrap since `on_open` returns a `Result<()>`.
        // If this call fails, it will only result in this connection disconnecting.
        println!("connected");


        loop {
            let mut lobby_id_option = self.lobby_id_mutex.lock().unwrap();
            let mutex_status = (*lobby_id_option).clone();
            match mutex_status {
                MutexStatus::Unmodified => {
                    *lobby_id_option = MutexStatus::InProgress;
                    let new_lobby = NewLobbyMessage {
                        max_players: 4,
                        password: String::from("eggs"),
                        lobby_name: String::from("brennan_test_lobby"),
                        player_name: String::from("bot_host")
                    };
                    let new_lobby_message = PalaceMessage::NewLobby(new_lobby);
                    let message_lobby = serde_json::to_vec(&new_lobby_message).unwrap();
                    println!("no lobby exists. Attempting to create a new one");
                    self.out.send(message_lobby);
                    break;
                },
                MutexStatus::InProgress => {
                    println!("lobby is in the process of being created");
                    continue;
                },
                MutexStatus::Finished(ref lobby_id_string) => {
                    self.lobby_id = Some(lobby_id_string.to_string());
                    // TODO: join lobby
                    println!("got lobby id from mutex");
                    let join_lobby_message = PalaceMessage::JoinLobby(JoinLobbyMessage{lobby_id : lobby_id_string.to_string(), player_name: String::from("brennan"), password: String::from("eggs")});
                    let json_join_lobby_message = serde_json::to_vec(&join_lobby_message).unwrap();
                    self.out.send(json_join_lobby_message);
                    break;
                }
            }
        }
    Ok(())
//        println!("resutl of send, {:?}",a);

    }

    //result of on_open is passed into this,
    // `on_message` is roughly equivalent to the Handler closure. It takes a `Message`
    // and returns a `Result<()>`.
    fn on_message(&mut self, msg: Message) -> ws::Result<()> {
        // Close the connection when we get a response from the server
        println!("Got message: {}", msg);
        let received_message = serde_json::from_slice::<PalaceOutMessage>(&msg.into_data()).unwrap();
        println!("message: {:?}", received_message);
        match received_message {
            PalaceOutMessage::NewLobbyResponse(received_message) => {
                println!("setting lobby id");
                let lobby_response = received_message.unwrap();
                self.player_id = Some(lobby_response.player_id.clone());
                self.lobby_id = Some(lobby_response.lobby_id.clone());

                let mut lobby_id_option = self.lobby_id_mutex.lock().unwrap();
                *lobby_id_option  = MutexStatus::Finished(self.lobby_id.clone().unwrap());
                println!("set status of creating lobby to finished");
                let join_lobby_message = PalaceMessage::JoinLobby(JoinLobbyMessage{lobby_id : lobby_response.lobby_id.clone(), player_name: String::from("brennan"), password: String::from("eggs")});
                let json_join_lobby_message = serde_json::to_vec(&join_lobby_message).unwrap();
                self.out.send(json_join_lobby_message);

            }
            PalaceOutMessage::LobbyList(received_message) => {
                // this doesn't really do anything
                println!("number of lobbies: {}", received_message.len());
            }
            PalaceOutMessage::JoinLobbyResponse(received_message) => {
                println!("connected to lobby!!");
            }
            PalaceOutMessage::PublicGameState(receieved_message) => {
                println!("testing3");
            }
            PalaceOutMessage::Hand(receieved_message) => {
                println!("testing4");
            }

        }
        Ok(())
    }

}

impl Client {

    fn do_stuff(&mut self) {

        println!("stuff");
        self.out.close(CloseCode::Normal);
    }

}



fn main() {
    println!("begin program");
    let lobby_id = Arc::new(Mutex::new(MutexStatus::Unmodified));

    let mut join_handle_vec = Vec::new();
    for x in 0..5 {
        let lobby_id_clone = lobby_id.clone();
        let handle = thread::spawn( move ||{
            println!("creating thread: {}", x);
            connect("ws://dev.brick.codes:3012", move |out| Client { out: out, player_id: None, player_index: None, lobby_id_mutex: lobby_id_clone.clone(), lobby_id: None, }).unwrap();
        });
        join_handle_vec.push(handle);
    }
    for handle in &mut join_handle_vec.into_iter() {
        handle.join().unwrap();
    }

//    let lobby_id1 = lobby_id.clone();
//    let lobby_id2 = lobby_id.clone();
//    // Now, instead of a closure, the Factory returns a new instance of our Handler.
//    let handle = thread::spawn( move ||{
//        connect("ws://dev.brick.codes:3012", move |out| Client { out: out, player_id: None, player_index: None, lobby_id_mutex: lobby_id1.clone(), lobby_id: None, }).unwrap();
//    });
//    let handle2 = thread::spawn(move ||{
//        connect("ws://dev.brick.codes:3012", move |out| Client { out: out, player_id: None, player_index: None, lobby_id_mutex: lobby_id2.clone(), lobby_id: None }).unwrap();
//    });
//    handle.join().unwrap();
//    handle2.join().unwrap();
    println!("end program");
}
//fn main() {
//    connect("ws://dev.brick.codes:3012", |out| {
//
//        let list_lobbies = PalaceMessage::ListLobbies;
//
//        let message = serde_json::to_vec(&list_lobbies).unwrap();
//
//        out.send(message).unwrap();
//        println!("here!");
//        move |msg: ws::Message| {
//            println!("TEsting");\

//            let msg_as_str = String::from_utf8(msg.clone().into_data()).unwrap();
//            println!("Got message: {}", msg_as_str);
//
//            let received_message = serde_json::from_slice::<PalaceOutMessage>(&msg.into_data()).unwrap();
//
//            println!("message: {:?}", received_message);
//            out.close(CloseCode::Normal)
//        }
//
//
//    }).unwrap()
//}


