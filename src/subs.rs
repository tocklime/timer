use crate::workout::FlatStatus;

#[derive(Clone)]
pub enum Event {
    Connect,
    Disconnect,
    StartWorkout,
    ToConfig,
    Beep{ freq : f32, dur: f64 }
}