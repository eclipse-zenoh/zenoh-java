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

impl From<Priority> for zenoh::qos::Priority {
    fn from(value: Priority) -> Self {
        match value {
            Priority::RealTime => zenoh::qos::Priority::RealTime,
            Priority::InteractiveHigh => zenoh::qos::Priority::InteractiveHigh,
            Priority::InteractiveLow => zenoh::qos::Priority::InteractiveLow,
            Priority::DataHigh => zenoh::qos::Priority::DataHigh,
            Priority::Data => zenoh::qos::Priority::Data,
            Priority::DataLow => zenoh::qos::Priority::DataLow,
            Priority::Background => zenoh::qos::Priority::Background,
        }
    }
}
