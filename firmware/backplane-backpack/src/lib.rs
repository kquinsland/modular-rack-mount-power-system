#![no_std]

#[cfg(not(feature = "board-rev-a"))]
compile_error!("select exactly one board feature; currently supported: board-rev-a");

pub mod board;

pub use board::BOARD;
