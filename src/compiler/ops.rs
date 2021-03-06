
use crate::{Symbol, Value};
use crate::data::{CompareOp};
use super::{EnumOption, Calculation, CompareValue, Binding, RemovalMode, ApplyTupleItem};

#[derive(Debug, Clone)]
pub enum Op {
    End,
    BeginNot {
        index: usize,
        sequence_len: usize,
    },
    EndNot {
        index: usize,
    },
    SearchAttributeBinding {
        binding: Binding,
        attribute: Symbol,
        value_binding: Binding,
    },
    RequireAttributeBinding {
        binding: Binding,
        attribute: Symbol,
        value_binding: Binding,
    },
    RequireAttributeValue {
        binding: Binding,
        attribute: Symbol,
        value: Value,
    },
    RequireAttribute {
        binding: Binding,
        attribute: Symbol,
    },
    AssertObjectBinding {
        binding: Binding,
    },
    CompareBinding {
        binding: Binding,
        value: Value,
    },
    UnpackTupleBinding {
        binding: Binding,
        values: Vec<TupleItem>,
    },
    MatchEnumBinding {
        binding: Binding,
        options: Vec<EnumOption>,
    },
    Calculation {
        binding: Binding,
        operation: Calculation,
    },
    CalculationCompare {
        binding: Binding,
        operation: Calculation,
    },
    Compare {
        comparison: Box<Comparison>,
    },
}

#[derive(Debug, Clone)]
pub struct Comparison {
    pub operator: CompareOp,
    pub left: CompareValue,
    pub right: CompareValue,
}

#[derive(Debug, Clone)]
pub enum TupleItem {
    Ignore,
    Bind(Binding),
    CompareBinding(Binding),
    CompareValue(Value),
}

#[derive(Debug, Clone)]
pub enum OpApply {
    CreateObject {
        binding: Binding,
    },
    CreateTuple {
        binding: Binding,
        items: Vec<ApplyTupleItem>,
    },
    AddBindingAttribute {
        binding: Binding,
        attribute: Symbol,
        value_binding: Binding,
    },
    RemoveBindingAttribute {
        binding: Binding,
        attribute: Symbol,
        value_binding: Binding,
        mode: RemovalMode,
    },
    AddValueAttribute {
        binding: Binding,
        attribute: Symbol,
        value: Value,
    },
    RemoveValueAttribute {
        binding: Binding,
        attribute: Symbol,
        value: Value,
        mode: RemovalMode,
    },
    Conditional {
        condition: Vec<Op>,
        then_apply: Vec<OpApply>,
        otherwise_apply: Vec<OpApply>,
    },
}
