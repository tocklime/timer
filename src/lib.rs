use seed::{prelude::*, *};
use serde::{Deserialize, Serialize};
use web_sys;
use web_sys::AudioContext;

mod mqtt_websocket;
mod routine;
mod workout;

use ulid::Ulid;
mod pages;
mod subs;

struct_urls!();
impl<'a> Urls<'a> {
    pub fn login(self) -> Url {
        self.base_url()
    }
    pub fn config(self) -> Url {
        self.base_url().add_path_part("config")
    }
    pub fn workout(self) -> Url {
        self.base_url().add_path_part("workout")
    }
}
// ------ ------
//     Model
// ------ ------
enum Page {
    Login(pages::login::Model),
    Config(pages::config::Model),
    Workout(pages::workout::Model),
}

struct Model {
    page: Page,
    audio_ctx: Option<AudioContext>,
    mqtt_connection: Option<mqtt_websocket::Model<AppMsg>>,
}
const TOPIC_PREFIX: &str = "/xcvyunaizrsemkt/timer-app/test";
impl Default for Model {
    fn default() -> Self {
        Self {
            page: Page::Login(pages::login::Model::init()),
            audio_ctx: web_sys::AudioContext::new().ok(),
            mqtt_connection: None,
        }
    }
}
impl Model {
    pub fn get_login(&self) -> &pages::login::Model {
        match &self.page {
            Page::Login(l) => l,
            Page::Config(c) => &c.login,
            Page::Workout(w) => &w.config.login,
        }
    }
    pub fn get_config(&self) -> Option<&pages::config::Model> {
        match &self.page {
            Page::Login(_) => None,
            Page::Config(c) => Some(c),
            Page::Workout(w) => Some(&w.config),
        }
    }
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
}

// ------ ------
//    Update
// ------ ------

#[derive(Serialize, Deserialize, Clone, Debug)]
enum AppMsg {
    LoginMsg(pages::login::Msg),
    ConfigMsg(pages::config::Msg),
    WorkoutMsg(pages::workout::Msg),
}

#[derive(Serialize, Deserialize, Debug)]
struct MqttMsg {
    msg: AppMsg,
    sender: Ulid,
}
enum Msg {
    InternalMsg(AppMsg),
    ExternalMsg(mqtt_websocket::ReceivedMsg<AppMsg>),
    MqttMsg(mqtt_websocket::Msg),
    Rendered(RenderInfo),
    HandleEvent(subs::Event),
}
fn update_app(msg: AppMsg, model: &mut Model, orders: &mut impl Orders<AppMsg>) {
    match (&mut model.page, &msg) {
        (Page::Config(c), AppMsg::ConfigMsg(m)) => {
            pages::config::update(m.clone(), c, &mut orders.proxy(AppMsg::ConfigMsg))
        }

        (Page::Login(l), AppMsg::LoginMsg(m)) => {
            pages::login::update(m.clone(), l, &mut orders.proxy(AppMsg::LoginMsg))
        }
        (Page::Workout(l), AppMsg::WorkoutMsg(m)) => {
            pages::workout::update(m.clone(), l, &mut orders.proxy(AppMsg::WorkoutMsg))
        }
        _ => {}
    }
}

fn update(msg: Msg, model: &mut Model, orders: &mut impl Orders<Msg>) {
    match msg {
        Msg::InternalMsg(msg2) => {
            if let Some(x) = &mut model.mqtt_connection {
                x.send_msg(&mut orders.proxy(Msg::MqttMsg), &msg2);
            }
            update_app(msg2, model, &mut orders.proxy(Msg::InternalMsg));
        }
        Msg::ExternalMsg(msg2) => {
            update_app(msg2.msg, model, &mut orders.proxy(Msg::InternalMsg));
        }
        Msg::MqttMsg(m) => {
            if let Some(mqtt) = &mut model.mqtt_connection {
                mqtt_websocket::Model::update(m, mqtt, &mut orders.proxy(Msg::MqttMsg));
            }
        }
        Msg::Rendered(_) => {
            if let Page::Workout(x) = &mut model.page {
                if let Some(b) = x.time_fn() {
                    orders.notify(b);
                }
            }
            orders.after_next_render(Msg::Rendered);
        }
        Msg::HandleEvent(e) => match e {
            subs::Event::Connect => {
                let login = model.get_login().clone();
                model.mqtt_connection = Some(mqtt_websocket::Model::new(
                    "wss://test.mosquitto.org:8081/mqtt",
                    &format!("{}{}", TOPIC_PREFIX, login.room),
                    &login.password,
                ));
                mqtt_websocket::connect(&mut orders.proxy(Msg::MqttMsg));
                model.page = Page::Config(crate::pages::config::Model::init(login));
            }
            subs::Event::Disconnect => {
                let login = model.get_login().clone();
                model.mqtt_connection = None;
                model.page = Page::Login(login);
            }
            subs::Event::StartWorkout => {
                if let Some(c) = model.get_config() {
                    if let Ok(r) = &c.routine {
                        model.page =
                            Page::Workout(crate::pages::workout::Model::init(c.clone(), r.clone()))
                    }
                }
            }
            subs::Event::ToConfig => {
                if let Some(c) = model.get_config() {
                    model.page = Page::Config(c.clone());
                }
            }
            subs::Event::Beep { freq, dur } => {
                model.beep(dur, freq);
            }
        },
    }
}

fn after_mount(_: Url, orders: &mut impl Orders<Msg>) -> AfterMount<Model> {
    orders.after_next_render(Msg::Rendered);
    //mqtt_websocket::connect(&mut orders.proxy(Msg::MqttMsg));
    orders.subscribe(Msg::ExternalMsg);
    orders.subscribe(Msg::HandleEvent);
    AfterMount::default()
}

// ------ ------
//     View
// ------ ------

fn view_app(model: &Model) -> Node<AppMsg> {
    match &model.page {
        Page::Login(m) => pages::login::view(m).map_msg(AppMsg::LoginMsg),
        Page::Config(m) => pages::config::view(m).map_msg(AppMsg::ConfigMsg),
        Page::Workout(m) => pages::workout::view(m).map_msg(AppMsg::WorkoutMsg),
    }
}
fn view(model: &Model) -> Node<Msg> {
    view_app(model).map_msg(Msg::InternalMsg)
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
