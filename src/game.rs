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

pub type CardTriplet = (Option<Card>, Option<Card>, Option<Card>);

#[derive(Copy, Clone, Debug, Deserialize, PartialEq,)]
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