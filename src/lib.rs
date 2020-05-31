use js_sys;
use seed::{prelude::*, *};
use wasm_bindgen::prelude::*;
use web_sys;
use web_sys::AudioContext;

mod workout;
use workout::*;
mod routine;

use lazy_static::lazy_static;

// ------ ------
//     Model
// ------ ------
enum RunningState {
    RunningSince(f64),
    PausedAfter(f64),
    Configure,
}
struct Model {
    config: String,
    routine: Result<Vec<FlatStatus>, String>,
    state: RunningState,
    routine_ix: usize,
    audio_ctx: Option<AudioContext>,
}

impl Model {
    fn elapsed(&self) -> f64 {
        let millis = match self.state {
            RunningState::RunningSince(start) => js_sys::Date::now() - start,
            RunningState::PausedAfter(p) => p,
            RunningState::Configure => 0.,
        };
        millis / 1000.
    }
    fn get_second_adjust(&self) -> f64 {
        match self.state {
            RunningState::RunningSince(_) => 1.,
            RunningState::PausedAfter(_) => 1.,
            RunningState::Configure => 0.,
        }
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

impl Default for Model {
    fn default() -> Self {
        let mctx = web_sys::AudioContext::new().ok();
        let mut s = Self {
            state: RunningState::Configure,
            config: routine::SEVEN.to_owned(),
            routine: Err("Not compiled yet".to_owned()),
            routine_ix: 0,
            audio_ctx: mctx,
        };
        s.routine = s.compile_config();
        s
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
    pub fn get_routine_item(&self, ix: usize) -> Option<&FlatStatus> {
        match &self.routine {
            Ok(vec) => match vec.get(ix) {
                None => Some(&END_STATUS),
                x => x,
            },
            Err(_) => None,
        }
    }
    pub fn current_routine_item(&self) -> Option<&FlatStatus> {
        self.get_routine_item(self.routine_ix)
    }
}

// ------ ------
//    Update
// ------ ------

#[derive(Clone)]
enum Msg {
    Rendered(RenderInfo),
    ChangeItem(usize),
    Go,
    ConfigChanged(String),
    ToConfig,
}

fn update(msg: Msg, model: &mut Model, orders: &mut impl Orders<Msg>) {
    match msg {
        Msg::ConfigChanged(new_c) => {
            model.config = new_c;
            model.routine = model.compile_config();
        }
        Msg::ToConfig => {
            model.state = RunningState::Configure;
        }
        Msg::Rendered(render_info) => {
            if let Some(d) = model.current_routine_item().and_then(|x| x.duration) {
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
                    let item = model
                        .current_routine_item()
                        .expect("Should have workout item when trying to beep");
                    let freq = if item.is_rest() { 440. } else { 880. };
                    model.beep(0.2, freq);
                }
                RunningState::PausedAfter(_) => model.state = RunningState::PausedAfter(1.),
                RunningState::Configure => (),
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
                RunningState::Configure => {
                    if model.routine.is_ok() {
                        model.state = RunningState::RunningSince(js_sys::Date::now());
                    }
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

fn view_item(class: &str, item: &FlatStatus, ix: usize) -> Node<Msg> {
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

fn view_workout(model: &Model) -> Node<Msg> {
    let curr = model.current_routine_item().expect("Expected OK routine");
    let next = model.get_routine_item(model.routine_ix + 1).unwrap();
    let time = match curr.duration {
        None => model.elapsed(),
        Some(d) => model.get_second_adjust() + (d as f64) - model.elapsed(),
    } as u64;
    let items = model.routine.as_ref().expect("Expected OK routine");
    div![
        // --- Seconds ---
        div![
            class! {"workout"},
            div![
                class! {"time", if curr.is_rest() {"rest"} else {"work"}},
                svg![
                    attrs![At::ViewBox=>"0 0 43 18"],
                    style![St::Width=>"100%"],
                    text![
                        attrs![At::X=>"21", At::Y=>"14.5"],
                        style!["text-anchor"=>"middle"],
                        workout::timer(time)
                    ]
                ],
                //workout::timer(time),
                ev(Ev::Click, |_| Msg::Go)
            ],
            view_item("curr", curr, model.routine_ix),
            view_item("next", next, model.routine_ix + 1),
            ul![
                class! {"workout-list"},
                items
                    .iter()
                    .enumerate()
                    .filter(|(_, x)| !x.is_rest())
                    .map(|(ix, i)| view_list_item(ix, i, model.routine_ix)),
                li!["Back to Config", ev(Ev::Click, |_| Msg::ToConfig)]
            ]
        ],
    ]
}
fn view_config(model: &Model) -> Node<Msg> {
    div![
        class! {"config"},
        p![class! {"help"}, "Workout thingy. Config below is written in Dhall. Errors or start button on the right. In the main workout view, click the time at the top to pause/resume. Click any other item to jump to that item in the sequence."],
        textarea![&model.config, input_ev(Ev::Input, Msg::ConfigChanged)],
        match &model.routine {
            Err(s) => pre![class! {"error"}, s],
            Ok(_) => button!["Start", ev(Ev::Click, |_| Msg::Go)],
        }
    ]
}

fn view(model: &Model) -> Node<Msg> {
    match model.state {
        RunningState::Configure => view_config(model),
        _ => view_workout(model),
    }
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
