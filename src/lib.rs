use js_sys;
use seed::{prelude::*, *};
use serde::{Deserialize, Serialize};
use web_sys;
use web_sys::AudioContext;


use lazy_static::lazy_static;

use mqtt::packet;
use mqtt::Decodable;
use mqtt::Encodable;
use packet::{Packet, VariablePacket, ConnectPacket, PingrespPacket};
use std::io::Cursor;

use ulid::Ulid;
use workoutstep::WorkoutStep;

mod workoutstep;

// ------ ------
//     Model
// ------ ------
enum RunningState {
    RunningSince(usize,f64),
    PausedAfter(usize,f64),
    Configure,
}

const TOPIC_PREFIX: &str = "/xcvyunaizrsemkt/timer-app/";


struct Model {
    config: String,
    routine: Result<Vec<WorkoutStep>, String>,
    state: RunningState,
    audio_ctx: Option<AudioContext>,
    mqtt_url: String,
    web_socket: Option<WebSocket>,
    web_socket_reconnector: Option<StreamHandle>,
    channel: String,
    next_pkid: u16,
    my_id: Ulid,
}

impl Model {
    fn elapsed(&self) -> f64 {
        let millis = match self.state {
            RunningState::RunningSince(_,start) => js_sys::Date::now() - start,
            RunningState::PausedAfter(_,p) => p,
            RunningState::Configure => 0.,
        };
        millis / 1000.
    }
    fn get_second_adjust(&self) -> f64 {
        match self.state {
            RunningState::RunningSince(_,_) => 1.,
            RunningState::PausedAfter(_,_) => 1.,
            RunningState::Configure => 0.,
        }
    }
    fn compile_config(&self) -> Result<Vec<WorkoutStep>, String> {
        WorkoutStep::from_csv(&self.config)
        .map_err(|c| format!("{}",c))
    }
    fn create_websocket(&self, orders: &impl Orders<Msg>) -> WebSocket {
        WebSocket::builder(&self.mqtt_url, orders)
            .on_open(|| Msg::WebSocketOpened)
            .on_message(Msg::WebSocketMsgReceived)
            .on_close(Msg::WebSocketClosed)
            .on_error(|| Msg::WebSocketFailed)
            .protocols(&["mqttv3.1"])
            .build_and_open()
            .unwrap()
    }
}

impl Default for Model {
    fn default() -> Self {
        let mut a = Self {
            state: RunningState::Configure,
            config: workoutstep::SEVEN.to_owned(),
            routine: Err("Not compiled yet".to_owned()),
            audio_ctx: web_sys::AudioContext::new().ok(),
            web_socket: None,
            web_socket_reconnector: None,
            mqtt_url: "wss://test.mosquitto.org:8081/mqtt".to_owned(),
            channel: "test".to_owned(),
            next_pkid: 1,
            my_id: Ulid::new(),
        };
        a.routine = a.compile_config();
        return a;
    }
}

lazy_static! {
    static ref END_STATUS: WorkoutStep = WorkoutStep {
        name: "END".to_string(),
        duration: None,
        rest_after: 0,
        rest_before: 0
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
    pub fn get_routine_item(&self, ix: usize) -> Option<&WorkoutStep> {
        match &self.routine {
            Ok(vec) => match vec.get(ix) {
                None => Some(&END_STATUS),
                x => x,
            },
            Err(_) => None,
        }
    }
    pub fn current_routine_ix(&self) -> Option<usize> {
        match self.state {
            RunningState::RunningSince(ix, _) => Some(ix),
            RunningState::PausedAfter(ix, _) => Some(ix),
            RunningState::Configure => None
        }
    }
    pub fn current_routine_item(&self) -> Option<&WorkoutStep> {
        self.current_routine_ix().and_then(|x|self.get_routine_item(x))
    }
}

// ------ ------
//    Update
// ------ ------

#[derive(Serialize, Deserialize, Clone)]
enum InternalMsg {
    ChangeItem(usize),
    Go,
    ConfigChanged(String),
    ToConfig,
}

#[derive(Serialize, Deserialize)]
struct MqttMsg {
    msg: InternalMsg,
    sender: Ulid,
}
#[derive(Eq, PartialEq)]
enum MsgSource {
    Internal,
    External,
}
enum Msg {
    Rendered(RenderInfo),
    InternalMsg(InternalMsg, MsgSource),
    WebSocketOpened,
    WebSocketClosed(CloseEvent),
    WebSocketMsgReceived(WebSocketMessage),
    WebSocketFailed,
    ReconnectWebSocket(usize),
    WebSocketSend(mqtt::packet::VariablePacket),
    MqttSubscribe,
}
fn imsg(im: InternalMsg) -> Msg {
    Msg::InternalMsg(im, MsgSource::Internal)
}

fn update(msg: Msg, model: &mut Model, orders: &mut impl Orders<Msg>) {
    match msg {
        Msg::InternalMsg(msg2, src) => {
            if src == MsgSource::Internal {
                let target =
                    mqtt::TopicName::new(format!("{}{}", TOPIC_PREFIX, model.channel)).unwrap();
                let pkid = model.next_pkid;
                model.next_pkid += 1;
                let qos = mqtt::packet::QoSWithPacketIdentifier::new(
                    mqtt::QualityOfService::Level0,
                    pkid,
                );
                let mqtt_msg = MqttMsg {
                    msg: msg2.clone(),
                    sender: model.my_id,
                };
                let json = serde_json::to_string(&mqtt_msg).unwrap();
                let pub_pkt = mqtt::packet::PublishPacket::new(target, qos, json);
                orders.send_msg(Msg::WebSocketSend(VariablePacket::PublishPacket(pub_pkt)));
            }
            match msg2 {
                InternalMsg::ConfigChanged(new_c) => {
                    model.config = new_c.clone(); //TODO: Why do I need to clone here?
                    model.routine = model.compile_config();
                }
                InternalMsg::ToConfig => {
                    model.state = RunningState::Configure;
                }
                InternalMsg::Go => {
                    model.beep(0.1, 880.);
                    match model.state {
                        RunningState::RunningSince(ix,start) => {
                            let done = js_sys::Date::now() - start;
                            model.state = RunningState::PausedAfter(ix,done);
                        }
                        RunningState::PausedAfter(ix,done) => {
                            let new_start = js_sys::Date::now() - done;
                            model.state = RunningState::RunningSince(ix,new_start);
                        }
                        RunningState::Configure => {
                            if model.routine.is_ok() {
                                model.state = RunningState::RunningSince(0,js_sys::Date::now());
                            }
                        }
                    }
                }
                InternalMsg::ChangeItem(new_ix) => {
                    match model.state {
                        RunningState::RunningSince(_,_) => {
                            model.state = RunningState::RunningSince(new_ix,js_sys::Date::now());
                            model.beep(0.2, 880.);
                        }
                        RunningState::PausedAfter(_,_) => model.state = RunningState::PausedAfter(new_ix,1.),
                        RunningState::Configure => (),
                    };
                }
            }
        }
        Msg::Rendered(render_info) => {
            if let Some(d) = model.current_routine_item().and_then(|x| x.duration) {
                let elapsed = model.elapsed();
                if elapsed > d as f64 {
                    orders.send_msg(imsg(InternalMsg::ChangeItem(model.current_routine_ix().unwrap() + 1)));
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
        Msg::WebSocketSend(pkt) => {
            let mut buffer = Vec::new();
            pkt.encode(&mut buffer).unwrap();
            model
                .web_socket
                .as_ref()
                .unwrap()
                .send_bytes(&buffer)
                .unwrap();
        }
        Msg::WebSocketOpened => {
            model.web_socket_reconnector = None;
            log!("WS Open");
            //send con packet.
            let client_id = model.my_id.to_string();
            let mut con_pkt = ConnectPacket::new("MQTT", client_id);
            con_pkt.set_keep_alive(30);
            con_pkt.set_clean_session(true);
            orders.send_msg(Msg::WebSocketSend(VariablePacket::ConnectPacket(con_pkt)));
        }
        Msg::WebSocketClosed(close_event) => {
            log!("WS Closed");
            if !close_event.was_clean() && model.web_socket_reconnector.is_none() {
                model.web_socket_reconnector = Some(
                    orders.stream_with_handle(streams::backoff(None, Msg::ReconnectWebSocket)),
                );
            }
        }
        Msg::MqttSubscribe => {
            let target =
                mqtt::TopicFilter::new(format!("{}{}", TOPIC_PREFIX, model.channel)).unwrap();
            let pkid = model.next_pkid;
            let qos = mqtt::QualityOfService::Level0;
            model.next_pkid += 1;
            let subpkt = mqtt::packet::SubscribePacket::new(pkid, vec![(target, qos)]);
            orders.send_msg(Msg::WebSocketSend(VariablePacket::SubscribePacket(subpkt)));
        }
        Msg::WebSocketMsgReceived(msg) => {
            log!("WS Message!");

            if msg.contains_text() {
                log!(format!("Text message: {}", msg.text().unwrap()));
            } else if msg.contains_blob() {
                log!(format!("blob message: {:?}", msg));
                let my_id = model.my_id;
                orders.perform_cmd(async move {
                    let bytes = msg.bytes().await.unwrap();
                    let mut dec_buf = Cursor::new(&bytes);
                    let decoded = mqtt::packet::VariablePacket::decode(&mut dec_buf).unwrap();
                    log!(format!("Decoded: {:?}", decoded));
                    match decoded {
                        packet::VariablePacket::ConnectPacket(_) => None,
                        packet::VariablePacket::ConnackPacket(_) => Some(Msg::MqttSubscribe),
                        packet::VariablePacket::PublishPacket(p) => {
                            //incoming message. try to decode.
                            let as_str = std::str::from_utf8(p.payload_ref()).unwrap();
                            let as_mqtt: MqttMsg = serde_json::from_str(as_str).unwrap();
                            if as_mqtt.sender != my_id {
                                Some(Msg::InternalMsg(as_mqtt.msg,MsgSource::External))
                            } else {
                                None
                            }
                        }
                        packet::VariablePacket::PubackPacket(_) => None,
                        packet::VariablePacket::PubrecPacket(_) => None,
                        packet::VariablePacket::PubrelPacket(_) => None,
                        packet::VariablePacket::PubcompPacket(_) => None,
                        packet::VariablePacket::PingreqPacket(_) => Some(Msg::WebSocketSend(VariablePacket::PingrespPacket(PingrespPacket::new()))),
                        packet::VariablePacket::PingrespPacket(_) => None,
                        packet::VariablePacket::SubscribePacket(_) => None,
                        packet::VariablePacket::SubackPacket(_) => None,
                        packet::VariablePacket::UnsubscribePacket(_) => None,
                        packet::VariablePacket::UnsubackPacket(_) => None,
                        packet::VariablePacket::DisconnectPacket(_) => None,
                    }
                });
            } else {
                log!(format!("Binary message? {:?}", msg));
            }
        }
        Msg::WebSocketFailed => {
            log!("WS Failed");
            if model.web_socket_reconnector.is_none() {
                model.web_socket_reconnector = Some(
                    orders.stream_with_handle(streams::backoff(None, Msg::ReconnectWebSocket)),
                );
            }
        }
        Msg::ReconnectWebSocket(retries) => {
            log!("Connect attempt: ", retries);
            model.web_socket = Some(model.create_websocket(orders));
        }
    }
}

fn after_mount(_: Url, orders: &mut impl Orders<Msg>) -> AfterMount<Model> {
    orders.after_next_render(Msg::Rendered);
    orders.send_msg(Msg::ReconnectWebSocket(0));
    AfterMount::default()
}

// ------ ------
//     View
// ------ ------

pub fn timer(duration: u32) -> String {
    format!("{}:{:02}", duration / 60, duration % 60)
}

fn view_item(class: &str, item: &WorkoutStep, ix: usize) -> Node<Msg> {
    div![
        class! {"item", class, "work"},
        div![class! {"duration"}, item.duration.map(timer)],
        &item.name,
        ev(Ev::Click, move |_| imsg(InternalMsg::ChangeItem(ix)))
    ]
}
fn view_list_item(ix: usize, item: &WorkoutStep, active_ix: usize) -> Node<Msg> {
    li![
        class! {"work",
        if active_ix > ix { "done" } else if active_ix == ix {"active"} else {"future"}},
        ev(Ev::Click, move |_| imsg(InternalMsg::ChangeItem(ix))),
        span![class! {"desc"}, &item.name],
        span![class! {"time"}, item.total_duration().map(timer)]
    ]
}

fn view_item_background(item: &WorkoutStep) -> Node<Msg> {
    let rb = item.rest_before;
    let ra = item.rest_after;
    let d = item.duration.unwrap_or(2 * (ra + rb));
    div![
        class! ["background"],
        style! {"grid-template-columns" => format!("{}fr {}fr {}fr", rb,d,ra)},
        div![class!["rest"]],
        div![class!["work"]],
        div![class!["rest"]],
    ]
}

fn view_workout(model: &Model) -> Node<Msg> {
    let curr_ix = model.current_routine_ix().expect("current ix");
    let current = model.get_routine_item(curr_ix).expect("Expected OK routine");
    let next = model.get_routine_item(curr_ix + 1).expect("Next routine item");
    let time = match current.duration {
        None => model.elapsed(),
        Some(d) => model.get_second_adjust() + (d as f64) - model.elapsed(),
    } as u32;
    let items = model.routine.as_ref().expect("Expected OK routine");
    div![
        // --- Seconds ---
        div![
            class! {"workout"},
            div![
                view_item_background(current),
                class! {"time"},
                svg![
                    attrs![At::ViewBox=>"0 0 43 18"],
                    style![St::Width=>"100%"],
                    text![
                        attrs![At::X=>"21", At::Y=>"14.5"],
                        style!["text-anchor"=>"middle"],
                        timer(time)
                    ]
                ],
                //workout::timer(time),
                ev(Ev::Click, |_| imsg(InternalMsg::Go))
            ],
            view_item("curr", current, curr_ix),
            view_item("next", next, curr_ix + 1),
            ul![
                class! {"workout-list"},
                items
                    .iter()
                    .enumerate()
                    .map(|(ix, i)| view_list_item(ix, i, curr_ix)),
                li![
                    "Back to Config",
                    ev(Ev::Click, |_| imsg(InternalMsg::ToConfig))
                ]
            ]
        ],
    ]
}
fn view_config(model: &Model) -> Node<Msg> {
    div![
        class! {"config"},
        p![class! {"help"}, "Workout thingy. Config below is written in Dhall. Errors or start button on the right. In the main workout view, click the time at the top to pause/resume. Click any other item to jump to that item in the sequence."],
        textarea![&model.config, input_ev(Ev::Input, |x| imsg(InternalMsg::ConfigChanged(x)))],
        match &model.routine {
            Err(s) => pre![class! {"error"}, s],
            Ok(_) => button!["Start", ev(Ev::Click, |_| imsg(InternalMsg::Go))],

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
