#![no_std]

#[cfg(test)]
extern crate std;

#[cfg(not(feature = "board-rev-a"))]
compile_error!("select exactly one board feature; currently supported: board-rev-a");

pub mod action_executor;
pub mod board;
pub mod command_service;
pub mod config_store;
pub mod fan;
pub mod status;
pub mod supervisor;

pub use board::BOARD;
