//! Opcode execution modules
//!
//! This module organizes opcode execution into logical categories,
//! making the VM easier to understand and maintain.
//!
//! Reference: $PHP_SRC_PATH/Zend/zend_vm_execute.h - opcode handlers

pub mod arithmetic;
pub mod array_ops;
pub mod bitwise;
pub mod comparison;
pub mod control_flow;
pub mod special;
