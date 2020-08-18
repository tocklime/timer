use seed::{prelude::*, *};
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct Model {
    pub room: String,
    pub password: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Msg {
    RoomChanged(String),
    PasswordChanged(String),
    Connect,
}

impl Model {
    pub fn init() -> Self {
        Self {
            room: "ABCD".to_owned(),
            password: "1234".to_owned(),
        }
    }
}
pub fn update(msg: Msg, model: &mut Model, orders: &mut impl Orders<Msg>) {
    match msg {
        Msg::RoomChanged(r) => {
            model.room = r;
        }
        Msg::PasswordChanged(p) => {
            model.password = p;
        }
        Msg::Connect => {
            orders.notify(crate::subs::Event::Connect);
        }
    }
}
pub fn view(model: &Model) -> Node<Msg> {
    div![
        C!["login"],
        p![class! {"help"}, "Please enter a room code and password."],
        input![
            attrs! {At::Value => model.room},
            input_ev(Ev::Input, Msg::RoomChanged)
        ],
        input![
            attrs! {At::Value => model.password},
            input_ev(Ev::Input, Msg::PasswordChanged)
        ],
        button!["Connect", ev(Ev::Click, |_| Msg::Connect)]
    ]
}
