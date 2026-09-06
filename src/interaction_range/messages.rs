use bevy::prelude::*;

#[derive(Message, Clone, Copy)]
pub struct FlagPickupMessage;

#[derive(Message, Clone, Copy)]
pub struct FlagDropMessage;
