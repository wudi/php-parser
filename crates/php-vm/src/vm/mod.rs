#![allow(dead_code)]

mod array_access;
pub mod assign_op;
mod callable;
mod class_resolution;
pub mod engine;
mod error_construction;
mod error_formatting;
pub mod executor;
pub mod frame;
mod frame_helpers;
pub mod inc_dec;
pub mod opcode;
mod opcode_executor;
mod opcodes;
pub mod stack;
mod stack_helpers;
mod superglobal;
mod type_conversion;
mod value_extraction;
mod variable_ops;
mod visibility;
