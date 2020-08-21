use seed::{prelude::*, *};
use serde::{Deserialize, Serialize};
use web_sys;
use web_sys::AudioContext;

mod mqtt_websocket;
mod routine;
mod workout;

use serde_json::Value;
use ulid::Ulid;
mod pages;
mod subs;
use chrono::{Utc,DateTime};

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
    Login,
    Workout(pages::workout::Model),
}

struct Model {
    page: Page,
    login: pages::login::Model,
    audio_ctx: Option<AudioContext>,
    mqtt_connection: Option<mqtt_websocket::Model<crate::pages::workout::PublishedModel>>,
    server_time_delta: i64,
    server_deltas: Vec<i64>,
}
const TOPIC_PREFIX: &str = "/xcvyunaizrsemkt/timer-app/test";

impl Default for Model {
    fn default() -> Self {
        Self {
            page: Page::Login,
            login: pages::login::Model::init(),
            audio_ctx: web_sys::AudioContext::new().ok(),
            mqtt_connection: None,
            server_time_delta: 0,
            server_deltas: Vec::new(),
        }
    }
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
}

// ------ ------
//    Update
// ------ ------

#[derive(Serialize, Deserialize, Clone, Debug)]
enum AppMsg {
    LoginMsg(pages::login::Msg),
    WorkoutMsg(pages::workout::Msg),
}

#[derive(Serialize, Deserialize, Debug)]
struct MqttMsg {
    msg: AppMsg,
    sender: Ulid,
}
enum Msg {
    InternalMsg(AppMsg),
    ExternalMsg(mqtt_websocket::ReceivedMsg<crate::pages::workout::PublishedModel>),
    MqttMsg(mqtt_websocket::Msg),
    Rendered(RenderInfo),
    HandleEvent(subs::Event),
    SetServerDelta(i64),
}
fn update_app(msg: AppMsg, model: &mut Model, orders: &mut impl Orders<AppMsg>) {
    match (&mut model.page, &msg) {
        (Page::Login, AppMsg::LoginMsg(m)) => pages::login::update(
            m.clone(),
            &mut model.login,
            &mut orders.proxy(AppMsg::LoginMsg),
        ),
        (Page::Workout(l), AppMsg::WorkoutMsg(m)) => {
            pages::workout::update(m.clone(), l, &mut orders.proxy(AppMsg::WorkoutMsg))
        }
        _ => {}
    }
}

fn update(msg: Msg, model: &mut Model, orders: &mut impl Orders<Msg>) {
    match msg {
        Msg::InternalMsg(msg2) => {
            update_app(msg2, model, &mut orders.proxy(Msg::InternalMsg));
        }
        Msg::ExternalMsg(msg2) => {
            if let Page::Workout(m) = &mut model.page {
                crate::pages::workout::update(crate::pages::workout::Msg::ExternalUpdate(msg2.msg), m, &mut orders.proxy(Msg::InternalMsg).proxy(AppMsg::WorkoutMsg));
            }
        }
        Msg::MqttMsg(m) => {
            if let Some(mqtt) = &mut model.mqtt_connection {
                mqtt_websocket::Model::update(m, mqtt, &mut orders.proxy(Msg::MqttMsg));
            }
        }
        Msg::Rendered(_) => {
            if let Page::Workout(x) = &mut model.page {
                for ev in x.time_fn() {
                    orders.notify(ev);
                }
            }
            orders.after_next_render(Msg::Rendered);
        }
        Msg::HandleEvent(e) => match e {
            subs::Event::Connect => {
                model.mqtt_connection = Some(mqtt_websocket::Model::new(
                    "wss://test.mosquitto.org:8081/mqtt",
                    &format!("{}{}", TOPIC_PREFIX, &model.login.room),
                    &model.login.password,
                ));
                mqtt_websocket::connect(&mut orders.proxy(Msg::MqttMsg));
                model.page = Page::Workout(crate::pages::workout::Model::init());
            }
            subs::Event::Disconnect => {
                model.mqtt_connection = None;
                model.page = Page::Login;
            }
            subs::Event::Beep { freq, dur } => {
                model.beep(dur, freq);
            }
            subs::Event::PublishedStateUpdated => {
            }
        },
        Msg::SetServerDelta(d) => {
            model.server_deltas.push(d);
            model.server_time_delta =
                model.server_deltas.iter().sum::<i64>() / model.server_deltas.len() as i64;
            if model.server_deltas.len() < 5 {
                orders.perform_cmd(request_time());
            } else {
                log!(model.server_time_delta);
            }
        }
    }
}

const TIME_URL: &str = "https://worldtimeapi.org/api/timezone/Etc/UTC";

async fn request_time() -> Option<Msg> {
    let before = Utc::now();
    let res = Request::new(TIME_URL)
        .method(Method::Get)
        .fetch()
        .await
        .ok()?;
    let now = Utc::now();
    let text = res.text().await.ok()?;
    let server_json: Value = serde_json::from_str(&text).ok()?;
    let server_now_str = server_json["utc_datetime"].as_str()?;
    let server_now = DateTime::parse_from_rfc3339(server_now_str).ok()?.with_timezone(&Utc);
    let one_way_time = (now - before) / 2;
    let diff = server_now - now + one_way_time;
    Some(Msg::SetServerDelta(diff.num_milliseconds()))
}

fn after_mount(_: Url, orders: &mut impl Orders<Msg>) -> AfterMount<Model> {
    orders.perform_cmd(request_time());
    orders.after_next_render(Msg::Rendered);
    orders.subscribe(Msg::ExternalMsg);
    orders.subscribe(Msg::HandleEvent);
    AfterMount::default()
}

// ------ ------
//     View
// ------ ------

fn view_app(model: &Model) -> Node<AppMsg> {
    match &model.page {
        Page::Login => pages::login::view(&model.login).map_msg(AppMsg::LoginMsg),
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
