use bevy::prelude::*;

#[derive(Message, Clone, Copy)]
pub struct FlagPickupMessage {
    pub agent_id: u32,
}

#[derive(Message, Clone, Copy)]
pub struct FlagDropMessage {
    pub agent_id: u32,
}
