use itertools::Itertools;
use js_sys;
use seed::{prelude::*, *};

mod workout;
// ------ ------
//     Init
// ------ ------

fn init(_url: Url, _orders: &mut impl Orders<Msg>) -> Model {
    Model {
        timer_handle: None,
        seconds: 0,
        started: 0.0,
        elapsed: 0.0,
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
            model.elapsed = (js_sys::Date::now() - model.started) / 1000.0;
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
