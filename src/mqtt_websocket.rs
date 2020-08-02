use mqtt::{
    packet::{self, ConnectPacket, PingrespPacket, VariablePacket},
    Decodable, Encodable, TopicName,
};
use packet::Packet;
use seed::{log, prelude::*};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{io::Cursor, marker::PhantomData};
use ulid::Ulid;

pub enum Msg {
    WebSocketOpened,
    WebSocketClosed(CloseEvent),
    WebSocketMsgReceived(WebSocketMessage),
    WebSocketFailed,
    ReconnectWebSocket(usize),
    WebSocketSend(mqtt::packet::VariablePacket),
    MqttSubscribe,
}
#[derive(Clone)]
pub struct ReceivedMsg<T>
where
    T: Clone,
{
    pub msg: T,
}
#[derive(Serialize, Deserialize)]
struct MqttWrap {
    msg: String,
    sender: Ulid,
}
pub struct Model<T>
where
    T: DeserializeOwned,
{
    id: Ulid,
    url: String,
    topic: String,
    web_socket: Option<WebSocket>,
    web_socket_reconnector: Option<StreamHandle>,
    phantom: PhantomData<T>,
}
impl<T> Model<T>
where
    T: 'static + DeserializeOwned + Clone + Serialize,
{
    pub fn new(url: &str, topic: &str) -> Self {
        let id = Ulid::new();
        Self {
            id,
            url: url.to_owned(),
            topic: topic.to_owned(),
            web_socket: None,
            web_socket_reconnector: None,
            phantom: PhantomData,
        }
    }
    pub fn create_websocket(&mut self, orders: &impl Orders<Msg>) -> WebSocket {
        let ws = WebSocket::builder(&self.url, orders)
            .on_open(|| Msg::WebSocketOpened)
            .on_message(Msg::WebSocketMsgReceived)
            .on_close(Msg::WebSocketClosed)
            .on_error(|| Msg::WebSocketFailed)
            .protocols(&["mqttv3.1"])
            .build_and_open()
            .unwrap();
        return ws;
    }
    pub fn send_msg(&self, orders: &mut impl Orders<Msg>, msg : &T) {
        let qos = mqtt::packet::QoSWithPacketIdentifier::new(
            mqtt::QualityOfService::Level0,
            1,
        );
        let json1 = serde_json::to_string(msg).unwrap();
        let mqtt_msg = MqttWrap {
            msg: json1,
            sender: self.id,
        };
        let json2 = serde_json::to_string(&mqtt_msg).unwrap();
        let topic = TopicName::new(&self.topic).unwrap();
        let pub_pkt = mqtt::packet::PublishPacket::new(topic, qos, json2);
        orders.send_msg(Msg::WebSocketSend(VariablePacket::PublishPacket(pub_pkt)));
    }

    pub fn update(msg: Msg, model: &mut Self, orders: &mut impl Orders<Msg>) {
        match msg {
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
                let client_id = model.id.to_string();
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
                let target = mqtt::TopicFilter::new(&model.topic).unwrap();
                let qos = mqtt::QualityOfService::Level0;
                let subpkt = mqtt::packet::SubscribePacket::new(1, vec![(target, qos)]);
                orders.send_msg(Msg::WebSocketSend(VariablePacket::SubscribePacket(subpkt)));
            }
            Msg::WebSocketMsgReceived(msg) => {
                log!("WS Message!");

                if msg.contains_text() {
                    log!(format!("Text message: {}", msg.text().unwrap()));
                } else if msg.contains_blob() {
                    log!(format!("blob message: {:?}", msg));
                    let my_id = model.id;
                    let ac = orders.clone_app();
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
                                let as_mqtt_wrap: MqttWrap = serde_json::from_str(as_str).unwrap();
                                if as_mqtt_wrap.sender != my_id {
                                    let as_t: T = serde_json::from_str(&as_mqtt_wrap.msg).unwrap();
                                    ac.notify(ReceivedMsg { msg: as_t });
                                }
                                None
                            }
                            packet::VariablePacket::PubackPacket(_) => None,
                            packet::VariablePacket::PubrecPacket(_) => None,
                            packet::VariablePacket::PubrelPacket(_) => None,
                            packet::VariablePacket::PubcompPacket(_) => None,
                            packet::VariablePacket::PingreqPacket(_) => Some(Msg::WebSocketSend(
                                VariablePacket::PingrespPacket(PingrespPacket::new()),
                            )),
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
}

pub fn connect(orders: &mut impl Orders<Msg>) {
    orders.send_msg(Msg::ReconnectWebSocket(0));
}