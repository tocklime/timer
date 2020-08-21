
#[derive(Clone)]
pub enum Event {
    Connect,
    Disconnect,
    Beep{ freq : f32, dur: f64 },
    PublishedStateUpdated,
}