
use crate::{Symbol, Value};
use crate::data::{CompareOp};
use super::{EnumOption, Calculation, CompareValue};

#[derive(Debug, Clone)]
pub enum Op {
    BeginNot {
        index: usize,
        sequence_len: usize,
    },
    EndNot {
        index: usize,
    },
    SearchAttributeBinding {
        binding: usize,
        attribute: Symbol,
        value_binding: usize,
    },
    RequireAttributeBinding {
        binding: usize,
        attribute: Symbol,
        value_binding: usize,
    },
    RequireAttributeValue {
        binding: usize,
        attribute: Symbol,
        value: Value,
    },
    RequireAttribute {
        binding: usize,
        attribute: Symbol,
    },
    AssertObjectBinding {
        binding: usize,
    },
    CompareBinding {
        binding: usize,
        value: Value,
    },
    UnpackTupleBinding {
        binding: usize,
        values: Vec<TupleItem>,
    },
    MatchEnumBinding {
        binding: usize,
        options: Vec<EnumOption>,
    },
    Calculation {
        binding: usize,
        operation: Calculation,
    },
    Compare {
        operator: CompareOp,
        left: CompareValue,
        right: CompareValue,
    },
}

#[derive(Debug, Clone)]
pub enum TupleItem {
    Ignore,
    Bind(usize),
    CompareBinding(usize),
    CompareValue(Value),
}