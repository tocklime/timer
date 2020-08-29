use mqtt::{
    packet::{self, ConnectPacket, PingrespPacket, VariablePacket},
    Decodable, Encodable, TopicName,
};
use packet::{Packet, PingreqPacket};
use seed::{log, prelude::*};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{fmt::Debug, io::Cursor, marker::PhantomData};
use ulid::Ulid;

pub enum Msg {
    WebSocketOpened,
    WebSocketClosed(CloseEvent),
    WebSocketMsgReceived(WebSocketMessage),
    WebSocketBytesReceived(Vec<u8>),
    WebSocketFailed,
    ReconnectWebSocket(usize),
    MqttSubscribe,
    KeepAlive,
}
#[derive(Clone)]
pub struct ReceivedMsg<T>
where
    T: Clone,
{
    pub msg: T,
}
#[derive(Serialize, Deserialize, Debug)]
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
    password: String,
    phantom: PhantomData<T>,
    keep_alive_handle: Option<StreamHandle>,
}
impl<T> Model<T>
where
    T: 'static + DeserializeOwned + Clone + Serialize + Debug,
{
    pub fn encrypt(&self, data: &[u8]) -> Vec<u8> {
        //TODO: Implement encryption based on self.password.
        return data.to_vec();
    }
    pub fn decrypt(&self, data: &[u8]) -> Vec<u8> {
        //TODO: Implement decryption based on self.password.
        return data.to_vec();
    }

    pub fn new(url: &str, topic: &str, password: &str) -> Self {
        let id = Ulid::new();
        Self {
            id,
            url: url.to_owned(),
            topic: topic.to_owned(),
            web_socket: None,
            web_socket_reconnector: None,
            password: password.to_owned(),
            phantom: PhantomData,
            keep_alive_handle: None,
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
    pub fn send_msg(&self, msg: &T) {
        let qos = mqtt::packet::QoSWithPacketIdentifier::new(mqtt::QualityOfService::Level0, 1);
        let json1 = serde_json::to_string(msg).unwrap();
        let mqtt_msg = MqttWrap {
            msg: json1,
            sender: self.id,
        };
        let json2 = serde_json::to_string(&mqtt_msg).unwrap();
        let encrypted = self.encrypt(&json2.as_bytes());
        let topic = TopicName::new(&self.topic).unwrap();
        let pub_pkt = mqtt::packet::PublishPacket::new(topic, qos, encrypted);
        self.send_pkt(VariablePacket::PublishPacket(pub_pkt));
    }
    pub fn send_pkt(&self, pkt: VariablePacket) {
        let mut buffer = Vec::new();
        pkt.encode(&mut buffer).unwrap();
        match self.web_socket.as_ref() {
            Some(s) => match s.send_bytes(&buffer) {
                Ok(_) => {}
                Err(e) => log!("Failed to send", e),
            },
            None => log!("Cannot send message: no websocket."),
        }
    }

    pub fn update(msg: Msg, model: &mut Self, orders: &mut impl Orders<Msg>) {
        match msg {
            Msg::KeepAlive => {
                model.send_pkt(VariablePacket::PingreqPacket(PingreqPacket::new()));
            }
            Msg::WebSocketOpened => {
                model.web_socket_reconnector = None;
                log!("WS Open");
                //send con packet.
                let client_id = model.id.to_string();
                let mut con_pkt = ConnectPacket::new("MQTT", client_id);
                con_pkt.set_keep_alive(30);
                con_pkt.set_clean_session(true);
                model.send_pkt(VariablePacket::ConnectPacket(con_pkt));
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
                model.send_pkt(VariablePacket::SubscribePacket(subpkt));
            }
            Msg::WebSocketMsgReceived(msg) => {
                if msg.contains_text() {
                    log!(format!("Text message: {}", msg.text().unwrap()));
                } else if msg.contains_blob() {
                    orders.perform_cmd(async move {
                        let bytes = msg.bytes().await.unwrap();
                        Msg::WebSocketBytesReceived(bytes)
                    });
                } else {
                    log!(format!("Binary message? {:?}", msg));
                }
            }
            Msg::WebSocketBytesReceived(bytes) => {
                let decrypted = model.decrypt(&bytes);
                let mut dec_buf = Cursor::new(&decrypted);
                let decoded = mqtt::packet::VariablePacket::decode(&mut dec_buf).unwrap();
                if let packet::VariablePacket::PublishPacket(_) = decoded {
                } else {
                    log!("incoming packet: ", decoded);
                }
                match decoded {
                    packet::VariablePacket::ConnectPacket(_) => {}
                    packet::VariablePacket::ConnackPacket(_) => {
                        orders.send_msg(Msg::MqttSubscribe);
                        model.keep_alive_handle = Some(
                            orders.stream_with_handle(streams::interval(20000, || Msg::KeepAlive)),
                        );
                    }
                    packet::VariablePacket::PublishPacket(p) => {
                        //incoming message. try to decode.
                        let as_str = std::str::from_utf8(p.payload_ref()).unwrap();
                        let as_mqtt_wrap: MqttWrap = serde_json::from_str(as_str).unwrap();
                        let my_id = model.id;
                        if as_mqtt_wrap.sender != my_id {
                            let as_t: T = serde_json::from_str(&as_mqtt_wrap.msg).unwrap();
                            log!("Decoded mqtt message", as_t);
                            orders.notify(ReceivedMsg { msg: as_t });
                        } else {
                            log!("ignored incoming message from self");
                        }
                    }
                    packet::VariablePacket::PubackPacket(_) => {}
                    packet::VariablePacket::PubrecPacket(_) => {}
                    packet::VariablePacket::PubrelPacket(_) => {}
                    packet::VariablePacket::PubcompPacket(_) => {}
                    packet::VariablePacket::PingreqPacket(_) => {
                        model.send_pkt(VariablePacket::PingrespPacket(PingrespPacket::new()));
                    }
                    packet::VariablePacket::PingrespPacket(_) => {}
                    packet::VariablePacket::SubscribePacket(_) => {}
                    packet::VariablePacket::SubackPacket(_) => {}
                    packet::VariablePacket::UnsubscribePacket(_) => {}
                    packet::VariablePacket::UnsubackPacket(_) => {}
                    packet::VariablePacket::DisconnectPacket(_) => {}
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
