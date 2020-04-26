use itertools::Itertools;
use js_sys;
use seed::{prelude::*, *};

mod workout;
use workout::*;
// ------ ------
//     Init
// ------ ------

fn init(_url: Url, _orders: &mut impl Orders<Msg>) -> Model {
    Model {
        timer_handle: None,
        seconds: 0,
        started: 0.0,
        elapsed: 0.0,
        routine: Some(joe_wicks().describe()),
        routine_ix: 0,
    }
}

// ------ ------
//     Model
// ------ ------

struct Model {
    timer_handle: Option<StreamHandle>,
    seconds: u32,
    elapsed: f64,
    started: f64,
    routine: Option<Vec<FlatStatus>>,
    routine_ix: usize
}

impl Model {
    pub fn current_routine_item(&self) -> Option<&FlatStatus> {
        self.routine.as_ref().and_then(|v| v.get(self.routine_ix))
    }
}

// ------ ------
//    Update
// ------ ------

enum Msg {
    StartTimer,
    StopTimer,
    OnTick,
}

fn update(msg: Msg, model: &mut Model, orders: &mut impl Orders<Msg>) {
    match msg {
        Msg::StartTimer => {
            model.timer_handle =
                Some(orders.stream_with_handle(streams::interval(100, || Msg::OnTick)));
            model.started = js_sys::Date::now();
            model.elapsed = 0.0;
        }
        Msg::StopTimer => {
            model.timer_handle = None;
        }
        Msg::OnTick => {
            model.seconds += 1;
            model.elapsed = (js_sys::Date::now() - model.started) / 100.0;
            if let Some(c) = model.current_routine_item() {
                let end = c.absolute_start_time + c.duration;
                if model.elapsed > end.into() {
                    model.routine_ix += 1;
                }
            }
        }
    }
}

// ------ ------
//     View
// ------ ------

fn view(model: &Model) -> impl IntoNodes<Msg> {
    let centered_column = style! {
        St::Display => "flex",
        St::FlexDirection => "column",
        St::AlignItems => "center"
    };
    let curr = model.current_routine_item().unwrap();

    div![
        centered_column.clone(),
        // --- Seconds ---
        div![
            style! {St::Display => "flex"},
            with_spaces(vec![
                div!["Seconds: ", model.seconds, " elapsed: ", model.elapsed],
                button![ev(Ev::Click, |_| Msg::StartTimer), "Start"],
                button![ev(Ev::Click, |_| Msg::StopTimer), "Stop"],
            ]),
            p![format!("Current item is {} for {}s",curr.name, curr.duration)]

        ],
    ]
}

fn with_spaces(nodes: Vec<Node<Msg>>) -> impl Iterator<Item = Node<Msg>> {
    nodes.into_iter().intersperse(span![
        style! {St::Width => rem(1), St::Display => "inline-block"}
    ])
}

// ------ ------
//     Start
// ------ ------

#[wasm_bindgen(start)]
pub fn start() {
    App::start("app", init, update, view);
}
