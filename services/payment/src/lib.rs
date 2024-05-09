pub mod api;
pub mod network;

pub mod hard_limit {
    pub const MAX_DB_CONNECTIONS: u32 = 1800u32;
    pub const MAX_SECONDS_DB_IDLE: u16 = 360u16;
}
