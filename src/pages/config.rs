use crate::{routine, workout::FlatStatus};
use serde::{Deserialize, Serialize};
use seed::{*, prelude::*};

#[derive(Clone)]
pub struct Model {
    pub config: String,
    pub routine: Result<Vec<FlatStatus>, String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Msg {
    ConfigChanged(String),
    Start,
    Disconnect,
}

impl Model {
    pub fn init() -> Self {
        let mut m = Self {
            config: routine::SEVEN.to_owned(),
            routine: Err("Not compiled yet".to_owned()),
        };
        m.routine = m.compile_config();
        m
    }
    fn compile_config(&self) -> Result<Vec<FlatStatus>, String> {
        let full = routine::TYPES.to_owned() + &self.config;
        let comp = serde_dhall::from_str(&full)
            .parse::<routine::Routine>()
            .map_err(|e| format!("{}", e))?;
        let fs = comp.to_full_workout()?;
        Ok(fs)
    }
}
pub fn update(msg: Msg, model: &mut Model, orders: &mut impl Orders<Msg>) {
    match msg {
        Msg::ConfigChanged(c) => {
            model.config = c;
            model.routine = model.compile_config();
        }
        Msg::Start => {orders.notify(crate::subs::Event::StartWorkout);}
        Msg::Disconnect => {orders.notify(crate::subs::Event::Disconnect);}
    }
}

pub fn view(model: &Model) -> Node<Msg> {
    div![
        class! {"config"},
        p![class! {"help"}, "Workout thingy. Config below is written in Dhall. Errors or start button on the right. In the main workout view, click the time at the top to pause/resume. Click any other item to jump to that item in the sequence."],
        textarea![&model.config, input_ev(Ev::Input, Msg::ConfigChanged)],
        match &model.routine {
            Err(s) => pre![class! {"error"}, s],
            Ok(_) => button!["Start", ev(Ev::Click, |_| Msg::Start)],
        }
    ]
}