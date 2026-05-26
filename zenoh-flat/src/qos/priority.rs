use prebindgen_proc_macro::prebindgen;

/// Message priority policy. Lower numeric value means higher priority.
#[prebindgen]
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Priority {
    RealTime = 1,
    InteractiveHigh = 2,
    InteractiveLow = 3,
    DataHigh = 4,
    Data = 5,
    DataLow = 6,
    Background = 7,
}

impl From<zenoh::qos::Priority> for Priority {
    fn from(value: zenoh::qos::Priority) -> Self {
        match value {
            zenoh::qos::Priority::RealTime => Priority::RealTime,
            zenoh::qos::Priority::InteractiveHigh => Priority::InteractiveHigh,
            zenoh::qos::Priority::InteractiveLow => Priority::InteractiveLow,
            zenoh::qos::Priority::DataHigh => Priority::DataHigh,
            zenoh::qos::Priority::Data => Priority::Data,
            zenoh::qos::Priority::DataLow => Priority::DataLow,
            zenoh::qos::Priority::Background => Priority::Background,
        }
    }
}
