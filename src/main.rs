extern crate ws;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate serde;
extern crate rand;

mod game;

use std::thread;
use std::time::Duration;
use std::sync::{Mutex, Arc};
use ws::{connect, Handler, Sender, Handshake, Message, CloseCode};
use serde::{Deserialize, Serialize, Deserializer, Serializer};
use rand::Rng;

const HAND_SIZE: usize = 6;

#[derive(Copy, Clone, Debug, Deserialize, Serialize, PartialEq)]
enum CardSuit {
    Clubs,
    Diamonds,
    Hearts,
    Spades,
}

const SUITS: [CardSuit; 4] = [
    CardSuit::Clubs,
    CardSuit::Diamonds,
    CardSuit::Hearts,
    CardSuit::Spades,
];

#[derive(Copy, Clone, Debug, Deserialize, Serialize, PartialEq)]
enum CardValue {
    Two,
    Three,
    Four,
    Five,
    Six,
    Seven,
    Eight,
    Nine,
    Ten,
    Jack,
    Queen,
    King,
    Ace,
}

const VALUES: [CardValue; 13] = [
    CardValue::Two,
    CardValue::Three,
    CardValue::Four,
    CardValue::Five,
    CardValue::Six,
    CardValue::Seven,
    CardValue::Eight,
    CardValue::Nine,
    CardValue::Ten,
    CardValue::Jack,
    CardValue::Queen,
    CardValue::King,
    CardValue::Ace,
];

#[derive(Copy, Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Card {
    value: CardValue,
    suit: CardSuit,
}

#[derive(Copy, Clone, Debug, Deserialize, PartialEq, )]
pub enum GamePhase {
    Setup,
    Play,
}

#[derive(Deserialize, Debug)]
pub struct PublicGameState {
    hands: Box<[usize]>,
    face_up_three: Box<[Box<[Card]>]>,
    face_down_three: Box<[u8]>,
    top_card: Option<Card>,
    pile_size: usize,
    cleared_size: usize,
    cur_phase: GamePhase,
    active_player: u8,
    last_cards_played: Box<[Card]>,
}

#[derive(Debug, Serialize)]
struct NewLobbyMessage {
    max_players: u8,
    password: String,
    lobby_name: String,
    player_name: String,
}

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Serialize)]
struct JoinLobbyMessage {
    lobby_id: String,
    player_name: String,
    password: String,
}

#[derive(Debug, Deserialize)]
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
struct StartGameMessage<'a> {
    lobby_id: &'a str,
    player_id: &'a str,

}

#[derive(Debug, Serialize)]
struct ChooseFaceupMessage {
    lobby_id: String,
    player_id: String,
    card_one: Card,
    card_two: Card,
    card_three: Card,
}

#[derive(Debug, Serialize)]
enum PalaceMessage<'a> {
    NewLobby(NewLobbyMessage),
    JoinLobby(JoinLobbyMessage),
    ListLobbies,
    StartGame(StartGameMessage<'a>),
    ChooseFaceup(ChooseFaceupMessage),
    MakePlay(MakePlayMessage),
    Reconnect(ReconnectMessage),
}

#[derive(Serialize, Debug)]
struct MakePlayMessage {
    cards: Box<[Card]>,
    lobby_id: String,
    player_id: String,
}

#[derive(Deserialize, Debug)]
struct HandResponse {
    hand: Box<[Card]>,
}

#[derive(Deserialize, Debug)]
struct PlayerJoinEvent {
    num_players: u8
}

#[derive(Deserialize, Debug)]
enum ChooseFaceupError {
    LobbyNotFound,
    GameNotStarted,
    PlayerNotFound,
    NotYourTurn,
}

#[derive(Deserialize, Debug)]
enum ReconnectError {
    LobbyNotFound,
    PlayerNotFound,
}

#[derive(Serialize, Debug)]
struct ReconnectMessage {
    player_id: String,
    lobby_id: String,
}

#[derive(Deserialize, Debug)]
enum MakePlayError {
    LobbyNotFound,
    GameNotStarted,
    PlayerNotFound,
    NotYourTurn,
}

#[derive(Debug, Deserialize)]
enum PalaceOutMessage {
    NewLobbyResponse(Result<NewLobbyResponse, NewLobbyError>),
    JoinLobbyResponse(Result<JoinLobbyResponse, JoinLobbyError>),
    LobbyList(Box<[LobbyDisplay]>),
    ChooseFaceupResponse(Result<HandResponse, ChooseFaceupError>),
    MakePlayResponse(Result<HandResponse,   MakePlayError>),
    ReconnectResponse(Result<(), ReconnectError>),
    PublicGameStateEvent(PublicGameState),
    GameStartedEvent(GameStartedEvent),
    PlayerJoinEvent(PlayerJoinEvent),
    StartGameResponse(Result<(), StartGameError>),
}

#[derive(Deserialize, Debug)]
struct GameStartedEvent {
    hand: Box<[Card]>,
    turn_number: u8,
}

#[derive(Deserialize, Debug)]
enum StartGameError {
    LobbyNotFound,
    NotLobbyOwner,
    LessThanTwoPlayers,
}


#[derive(Debug, Serialize, Deserialize)]
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
    Finished(String),
}


// Our Handler struct.
// Here we explicity indicate that the Client needs a Sender,
// whereas a closure captures the Sender for us automatically.
struct Client {
    out: Sender,
    player_id: Option<String>,
    player_index: Option<u8>,
    lobby_id_mutex: Arc<Mutex<MutexStatus>>,
    lobby_id: Option<String>,
    num_players: u8,
    is_host: bool,
    hand: Option<Box<[Card]>>,
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
                    self.is_host = true;
                    *lobby_id_option = MutexStatus::InProgress;
                    let new_lobby = NewLobbyMessage {
                        max_players: 4,
                        password: String::from("eggs"),
                        lobby_name: String::from("brennan_test_lobby"),
                        player_name: String::from("bot_host"),
                    };
                    let new_lobby_message = PalaceMessage::NewLobby(new_lobby);
                    let message_lobby = serde_json::to_vec(&new_lobby_message).unwrap();
                    println!("no lobby exists. Attempting to create a new one");
                    self.out.send(message_lobby);
                    break;
                }
                MutexStatus::InProgress => {
                    println!("lobby is in the process of being created");
                    continue;
                }
                MutexStatus::Finished(ref lobby_id_string) => {
                    self.lobby_id = Some(lobby_id_string.to_string());
                    // TODO: join lobby
                    println!("got lobby id from mutex");
                    let join_lobby_message = PalaceMessage::JoinLobby(JoinLobbyMessage { lobby_id: lobby_id_string.to_string(), player_name: String::from("brennan"), password: String::from("eggs") });
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
                *lobby_id_option = MutexStatus::Finished(self.lobby_id.clone().unwrap());
                println!("set status of creating lobby to finished");
            }
            PalaceOutMessage::LobbyList(received_message) => {
                // this doesn't really do anything
                println!("number of lobbies: {}", received_message.len());
            }
            PalaceOutMessage::JoinLobbyResponse(received_message) => {
                let lobby_response = received_message.unwrap();
                self.player_id = Some(lobby_response.player_id.clone());
                println!("Set player id, connected to lobby");

            }
            PalaceOutMessage::PublicGameStateEvent(received_message) => {
                println!("testing3");

                if self.player_index.unwrap() == received_message.active_player {
                    println!("it's my turn");
                    self.take_turn();
                }
            }
            PalaceOutMessage::ReconnectResponse(received_message) => {
                let reconnect_response = received_message.expect("Failed to reconnect");
                println!("reconnected");
            }
            PalaceOutMessage::MakePlayResponse(received_message) => {
                let hand_response = received_message.expect("probably played an invalid card");
                self.hand = Some(Box::new(hand_response).hand.clone());
                println!("made play");
            }
            PalaceOutMessage::ChooseFaceupResponse(received_message) => {
                let hand_response = received_message.expect("probably played an invalid card for faceup");
                println!("chose faceup");
            }
            PalaceOutMessage::GameStartedEvent(received_message) => {
                self.player_index = Some(received_message.turn_number);
                self.hand = Some(received_message.hand);
            }
            PalaceOutMessage::PlayerJoinEvent(received_message) => {
                if self.is_host {
                    println!("host");
                }
                if self.is_host && received_message.num_players == self.num_players {
                    println!("game is ready to begin!");
                    let start_game_details = StartGameMessage {
                        lobby_id : self.lobby_id.as_ref().unwrap(),
                        player_id : self.player_id.as_ref().unwrap(),
                    };

                    let start_game_message = PalaceMessage::StartGame(start_game_details);
                    let json_message = serde_json::to_vec(&start_game_message).unwrap();
                    self.out.send(json_message);
                }

            }
            PalaceOutMessage::StartGameResponse(received_message) => {
                received_message.expect("game didn't start");
            }

        }
        Ok(())
    }
}

impl Client {
    fn take_turn(&mut self) {
        println!("taking turn");
//        let temp_hand = self.hand.unwrap().clone();
        let random_card = rand::thread_rng().choose(self.hand.as_ref().unwrap()).unwrap();
        let play_details = MakePlayMessage {
            cards: vec![*random_card].into_boxed_slice(),
            lobby_id: self.lobby_id.clone().unwrap(),
            player_id: self.player_id.clone().unwrap(),
        };
        let play_message = PalaceMessage::MakePlay(play_details);
        let json_message = serde_json::to_vec(&play_message).unwrap();
        println!("sending play!");
        self.out.send(json_message);
    }
}


fn main() {
    println!("begin program");

    let NUM_PLAYERS = 4;

    let lobby_id = Arc::new(Mutex::new(MutexStatus::Unmodified));

    let mut join_handle_vec = Vec::new();
    for i in 0..NUM_PLAYERS {
        let lobby_id_clone = lobby_id.clone();
        let handle = thread::spawn(move || {
            println!("creating thread: {}", i);
            connect("ws://dev.brick.codes:3012", move |out| Client {
                out: out,
                player_id: None,
                player_index: None,
                lobby_id_mutex: lobby_id_clone.clone(),
                lobby_id: None,
                num_players: NUM_PLAYERS,
                is_host: false,
                hand: None,
            }).unwrap();
        });
        join_handle_vec.push(handle);
    }
    for handle in join_handle_vec.into_iter() {
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


