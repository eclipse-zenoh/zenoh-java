use prebindgen_proc_macro::prebindgen;

/// Reliability policy for publications/subscriptions.
#[prebindgen]
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reliability {
    BestEffort = 0,
    Reliable = 1,
}

impl From<zenoh::qos::Reliability> for Reliability {
    fn from(value: zenoh::qos::Reliability) -> Self {
        match value {
            zenoh::qos::Reliability::BestEffort => Reliability::BestEffort,
            zenoh::qos::Reliability::Reliable => Reliability::Reliable,
        }
    }
}

impl From<Reliability> for zenoh::qos::Reliability {
    fn from(value: Reliability) -> Self {
        match value {
            Reliability::BestEffort => zenoh::qos::Reliability::BestEffort,
            Reliability::Reliable => zenoh::qos::Reliability::Reliable,
        }
    }
}
