#![no_std]

#[cfg(test)]
extern crate std;

#[cfg(not(any(feature = "board-rev-a", feature = "board-rev-b")))]
compile_error!("select exactly one board feature: board-rev-a or board-rev-b");
#[cfg(all(feature = "board-rev-a", feature = "board-rev-b"))]
compile_error!("board-rev-a and board-rev-b are mutually exclusive");

pub mod action_executor;
pub mod board;
pub mod command_service;
pub mod config_store;
pub mod fan;
pub mod status;
pub mod supervisor;

pub use board::BOARD;
