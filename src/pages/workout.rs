use crate::workout::FlatStatus;
use seed::{prelude::*, *};

use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};

pub struct Model {
    pub config: crate::pages::config::Model,
    state: RunningState,
    pub routine: Vec<FlatStatus>,
    last_update: f64,
    routine_ix: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Msg {
    ChangeItem(usize),
    Go,
    ToConfig,
}
enum RunningState {
    RunningSince(f64),
    PausedAfter(f64),
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
    pub fn init(config : crate::pages::config::Model, routine: Vec<FlatStatus> ) -> Self {
        let now = js_sys::Date::now();
        Self {
            config: config,
            state: RunningState::RunningSince(now),
            routine: routine,
            last_update: now,
            routine_ix: 0
        }
    }
    fn elapsed(&self) -> f64 {
        let millis = match self.state {
            RunningState::RunningSince(start) => if self.last_update > start {self.last_update - start} else {log!("XXX",self.last_update, start); 0.},
            RunningState::PausedAfter(p) => p,
        };
        millis / 1000.
    }
    pub fn get_routine_item(&self, ix: usize) -> &FlatStatus {
        self.routine.get(ix).unwrap_or(&END_STATUS)
    }
    pub fn current_routine_item(&self) -> &FlatStatus {
        self.get_routine_item(self.routine_ix)
    }
    pub fn goto_item(&mut self, new_ix: usize) -> Option<crate::subs::Event> {
        self.routine_ix = new_ix;
        match self.state {
            RunningState::RunningSince(_) => {
                self.state = RunningState::RunningSince(js_sys::Date::now());
                let item = self.current_routine_item();
                let freq = if item.is_rest() { 440. } else { 880. };
                Some(crate::subs::Event::Beep { freq, dur: 0.2 })
            }
            RunningState::PausedAfter(_) => {
                self.state = RunningState::PausedAfter(1.);
                None
            }
        }
    }
    pub fn time_fn(&mut self) -> Option<crate::subs::Event> {
        if let Some(d) = self.current_routine_item().duration {
            let elapsed = self.elapsed();
            let remaining = d as f64 - elapsed;
            if remaining <= 0. {
                self.goto_item(self.routine_ix + 1)
            } else {
                self.last_update = js_sys::Date::now();
                let new_elapsed = self.elapsed();
                let remaining_now = d as f64 - new_elapsed;
                if remaining_now < 3. {
                    let whole_rem_now = remaining_now as u64;
                    let whole_rem_before = remaining as u64;
                    if whole_rem_before != whole_rem_now {
                        return Some(crate::subs::Event::Beep {
                            freq: 440.,
                            dur: 0.1,
                        });
                    }
                }
                None
            }
        } else {
            self.last_update = js_sys::Date::now();
            None
        }
    }
}
pub fn update(msg: Msg, model: &mut Model, orders: &mut impl Orders<Msg>) {
    match msg {
        Msg::Go => {
            orders.notify(crate::subs::Event::Beep {
                freq: 880.,
                dur: 0.1,
            });
            match model.state {
                RunningState::RunningSince(start) => {
                    let done = js_sys::Date::now() - start;
                    model.state = RunningState::PausedAfter(done);
                }
                RunningState::PausedAfter(done) => {
                    let new_start = js_sys::Date::now() - done;
                    model.state = RunningState::RunningSince(new_start);
                }
            }
        }
        Msg::ChangeItem(new_ix) => {
            if let Some(b) = model.goto_item(new_ix){
                orders.notify(b);
            }
        }
        Msg::ToConfig => {
            orders.notify(crate::subs::Event::ToConfig);
        }
    }
}

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

pub fn view(model: &Model) -> Node<Msg> {
    let current = model.current_routine_item();
    let next = model.get_routine_item(model.routine_ix + 1);
    let time = match current.duration {
        None => model.elapsed(),
        Some(d) => ((d as f64) - model.elapsed()).ceil(),
    } as u64;
    let items = &model.routine;
    div![
        // --- Seconds ---
        div![
            class! {"workout"},
            div![
                class! {"time", if current.is_rest() {"rest"} else {"work"}},
                svg![
                    attrs![At::ViewBox=>"0 0 43 18"],
                    style![St::Width=>"100%"],
                    text![
                        attrs![At::X=>"21", At::Y=>"14.5"],
                        style!["text-anchor"=>"middle"],
                        crate::workout::timer(time)
                    ]
                ],
                //workout::timer(time),
                ev(Ev::Click, |_| Msg::Go)
            ],
            view_item("curr", current, model.routine_ix),
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
