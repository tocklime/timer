use js_sys;
use web_sys;
use wasm_bindgen::prelude::*;
use web_sys::{OscillatorNode, AudioContext};
use seed::{prelude::*, *};

mod workout;
use workout::*;
use std::convert::TryInto;

use lazy_static::lazy_static;

// ------ ------
//     Model
// ------ ------
enum RunningState{
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
 static ref END_STATUS : FlatStatus = FlatStatus {
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
        }else {
            None
        }
    }
    pub fn get_routine_item(&self, ix: usize) -> &FlatStatus {
        match self.routine.get(ix) {
            None => &END_STATUS,
            Some(x) => x
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
    Rendered(Option<RenderTimestampDelta>),
    ChangeItem(i32),
    Start,
    Stop,
    Pause,
}

fn update(msg: Msg, model: &mut Model, orders: &mut impl Orders<Msg>) {
    match msg {
        Msg::Rendered(since_last) => {
            if let Some(d) = model.current_routine_item().duration {
                let elapsed = model.elapsed();
                if elapsed > d as f64 {
                    orders.send_msg(Msg::ChangeItem(1));
                }
                if let Some(s) = since_last {
                    let remaining_now = (d as f64) - elapsed;
                    if remaining_now < 3. {
                        let r : f64 = f64::from(s);
                        let remaining_before = remaining_now + r / 1000.;
                        let whole_rem_now = remaining_now as u64;
                        let whole_rem_before = remaining_before as u64;
                        if whole_rem_before != whole_rem_now {
                            model.beep(0.1,440.);
                        }
                    }
                }
            }
            orders.after_next_render(Msg::Rendered);
        }
        Msg::ChangeItem(delta) => {
            let new_ix = (model.routine_ix as i32) + delta;
            model.routine_ix = new_ix.try_into().unwrap();
            match model.state {
                RunningState::RunningSince(_) => {
                    model.state = RunningState::RunningSince(js_sys::Date::now());
                    let item = model.current_routine_item();
                    let freq = if item.is_rest() { 440. } else { 880. };
                    model.beep(0.2,freq);
                },
                RunningState::PausedAfter(_) => {model.state = RunningState::PausedAfter(1.)},
                RunningState::Stopped => {model.state = RunningState::Stopped},
            };
        }
        Msg::Start => {
            model.routine_ix = 0;
            model.state = RunningState::RunningSince(js_sys::Date::now());
            model.beep(0.2,440.);
        }
        Msg::Stop => {
            model.routine_ix = 0;
            model.state = RunningState::Stopped;
        }
        Msg::Pause => {
            model.beep(0.1,880.);
            match model.state {
                RunningState::RunningSince(start) => {
                    let done = js_sys::Date::now() - start;
                    model.state = RunningState::PausedAfter(done);
                },
                RunningState::PausedAfter(done) => {
                    let new_start = js_sys::Date::now() - done;
                    model.state = RunningState::RunningSince(new_start);
                },
                RunningState::Stopped => {},
            }
        }
    }
}

fn after_mount(_: Url, orders: &mut impl Orders<Msg>) -> AfterMount<Model> {
    orders.after_next_render(Msg::Rendered);
    AfterMount::default()
}

fn timer(duration: u64) -> String {
    format!("{}:{:02}",duration / 60, duration % 60)
}
// ------ ------
//     View
// ------ ------

fn view(model: &Model) -> impl IntoNodes<Msg> {
    let curr = model.current_routine_item();
    let next = model.get_routine_item(model.routine_ix + 1);
    let time = match curr.duration {
        None => {model.elapsed()},
        Some(d) => model.get_second_adjust() + (d as f64) - model.elapsed()
    } as u64;
    div![
        // --- Seconds ---
        div![
            class! {"workout"},
            div![
                class! {"time"},
                timer(time)
            ],
            div![
                class! {"controls"},
                button! [ "Start", ev(Ev::Click, |_| Msg::Start) ],
                button! [ "Pause", ev(Ev::Click, |_| Msg::Pause) ],
                button! [ "Stop", ev(Ev::Click, |_| Msg::Stop) ]
            ],
            div![
                class! {"curr"},
                format!(
                    "Current item is {} for {:?}s",
                    curr.name, curr.duration
                ),
            ],
            div![
                class! {"next"},
                format!( "Next item is {} for {:?}s", next.name, next.duration ),
                ev(Ev::Click, |_| Msg::ChangeItem(1))
            ]
        ],
    ]
}

// ------ ------
//     Start
// ------ ------

#[wasm_bindgen(start)]
pub fn start() {
    App::builder(update,view)
        .after_mount(after_mount)
        .build_and_start();
}
