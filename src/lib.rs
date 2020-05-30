use js_sys;
use seed::{prelude::*, *};
use wasm_bindgen::prelude::*;
use web_sys;
use web_sys::AudioContext;

mod workout;
use workout::*;

use crate::RunningState::RunningSince;
use lazy_static::lazy_static;

// ------ ------
//     Model
// ------ ------
enum RunningState {
    RunningSince(f64),
    PausedAfter(f64),
    Stopped,
}
struct Model {
    state: RunningState,
    routine: Vec<FlatStatus>,
    routine_ix: usize,
    audio_ctx: Option<AudioContext>,
}

impl Model {
    fn elapsed(&self) -> f64 {
        let millis = match self.state {
            RunningState::RunningSince(start) => js_sys::Date::now() - start,
            RunningState::PausedAfter(p) => p,
            RunningState::Stopped => 0.,
        };
        millis / 1000.
    }
    fn get_second_adjust(&self) -> f64 {
        match self.state {
            RunningState::RunningSince(_) => 1.,
            RunningState::PausedAfter(_) => 1.,
            RunningState::Stopped => 0.,
        }
    }
}

impl Default for Model {
    fn default() -> Self {
        let mctx = web_sys::AudioContext::new().ok();
        Self {
            state: RunningState::Stopped,
            routine: joe_wicks().describe(),
            routine_ix: 0,
            audio_ctx: mctx,
        }
    }
}

lazy_static! {
    static ref END_STATUS: FlatStatus = FlatStatus {
        name: "END".to_string(),
        this_rep: 1,
        total_reps: 1,
        duration: None,
    };
}
impl Model {
    pub fn beep(&self, duration: f64, frequency: f32) -> Option<()> {
        if let Some(ctx) = &self.audio_ctx {
            let osc = ctx.create_oscillator().ok()?;
            let gain = ctx.create_gain().ok()?;
            osc.connect_with_audio_node(&gain).ok()?;
            gain.connect_with_audio_node(&ctx.destination()).ok()?;
            osc.frequency().set_value(frequency);
            let now = self.audio_ctx.as_ref().unwrap().current_time();
            osc.start().ok()?;
            osc.stop_with_when(now + duration).ok()?;
            Some(())
        } else {
            None
        }
    }
    pub fn get_routine_item(&self, ix: usize) -> &FlatStatus {
        match self.routine.get(ix) {
            None => &END_STATUS,
            Some(x) => x,
        }
    }
    pub fn current_routine_item(&self) -> &FlatStatus {
        self.get_routine_item(self.routine_ix)
    }
}

// ------ ------
//    Update
// ------ ------

enum Msg {
    Rendered(RenderInfo),
    ChangeItem(usize),
    Go,
}

fn update(msg: Msg, model: &mut Model, orders: &mut impl Orders<Msg>) {
    match msg {
        Msg::Rendered(render_info) => {
            if let Some(d) = model.current_routine_item().duration {
                let elapsed = model.elapsed();
                if elapsed > d as f64 {
                    orders.send_msg(Msg::ChangeItem(model.routine_ix + 1));
                }
                if let Some(s) = render_info.timestamp_delta {
                    let remaining_now = (d as f64) - elapsed;
                    if remaining_now < 3. {
                        let r: f64 = f64::from(s);
                        let remaining_before = remaining_now + r / 1000.;
                        let whole_rem_now = remaining_now as u64;
                        let whole_rem_before = remaining_before as u64;
                        if whole_rem_before != whole_rem_now {
                            model.beep(0.1, 440.);
                        }
                    }
                }
            }
            orders.after_next_render(Msg::Rendered);
        }
        Msg::ChangeItem(new_ix) => {
            model.routine_ix = new_ix;
            match model.state {
                RunningState::RunningSince(_) => {
                    model.state = RunningState::RunningSince(js_sys::Date::now());
                    let item = model.current_routine_item();
                    let freq = if item.is_rest() { 440. } else { 880. };
                    model.beep(0.2, freq);
                }
                RunningState::PausedAfter(_) => model.state = RunningState::PausedAfter(1.),
                RunningState::Stopped => model.state = RunningState::Stopped,
            };
        }
        Msg::Go => {
            model.beep(0.1, 880.);
            match model.state {
                RunningState::RunningSince(start) => {
                    let done = js_sys::Date::now() - start;
                    model.state = RunningState::PausedAfter(done);
                }
                RunningState::PausedAfter(done) => {
                    let new_start = js_sys::Date::now() - done;
                    model.state = RunningState::RunningSince(new_start);
                }
                RunningState::Stopped => {
                    model.state = RunningState::RunningSince(js_sys::Date::now());
                }
            }
        }
    }
}

fn after_mount(_: Url, orders: &mut impl Orders<Msg>) -> AfterMount<Model> {
    orders.after_next_render(Msg::Rendered);
    AfterMount::default()
}

// ------ ------
//     View
// ------ ------

fn view_item(class: &str, model: &Model, ix: usize) -> Node<Msg> {
    let item = model.get_routine_item(ix);
    div![
        class! {"item", class, if item.is_rest() {"rest"} else {"work"}},
        div![class! {"reps"}, item.rep_str()],
        div![class! {"duration"}, item.dur_str()],
        &item.name,
        ev(Ev::Click, move |_| Msg::ChangeItem(ix))
    ]
}
fn view_list_item(ix: usize, item: &FlatStatus, active_ix: usize) -> Node<Msg> {
    li![
        class! {if item.is_rest() {"rest"} else {"work"}
        if active_ix > ix { "done" } else if active_ix == ix {"active"} else {"future"}},
        ev(Ev::Click, move |_| Msg::ChangeItem(ix)),
        span![class! {"desc"}, format!("{} {}", item.rep_str(), item.name)],
        span![class! {"time"}, item.dur_str()]
    ]
}

fn view(model: &Model) -> impl IntoNodes<Msg> {
    let curr = model.current_routine_item();
    let time = match curr.duration {
        None => model.elapsed(),
        Some(d) => model.get_second_adjust() + (d as f64) - model.elapsed(),
    } as u64;
    div![
        // --- Seconds ---
        div![
            class! {"workout"},
            div![
                class! {"time", if curr.is_rest() {"rest"} else {"work"}},
                workout::timer(time),
                ev(Ev::Click, |_| Msg::Go)
            ],
            view_item("curr", model, model.routine_ix),
            view_item("next", model, model.routine_ix + 1),
            ul![
                class! {"workout-list"},
                model
                    .routine
                    .iter()
                    .enumerate()
                    .map(|(ix, i)| view_list_item(ix, i, model.routine_ix))
            ]
        ],
    ]
}

// ------ ------
//     Start
// ------ ------

#[wasm_bindgen(start)]
pub fn start() {
    App::builder(update, view)
        .after_mount(after_mount)
        .build_and_start();
}
