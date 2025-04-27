// src/events.rs

/// Standard event types shared by all modules
pub enum BruteEvent {
    Info(String),
    Fail(String),
    Error(String),
    Success {
        protocol: String,
        ip: String,
        username: String,
        password: String,
        port: u16,
    },
}
