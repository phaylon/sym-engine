
use crate::{Value, Symbol};
use crate::data::{CompareOp};
use super::{Binding, EnumOption, Calculation, CompareValue, RemovalMode, ApplyTupleItem};

#[derive(Debug, Clone)]
pub enum CfgOpSelect {
    AssertObjectBinding {
        binding: Binding,
    },
    CompareBinding {
        binding: Binding,
        value: Value,
    },
    TupleBinding {
        binding: Binding,
        values: Vec<OpenTupleItem>,
    },
    EnumBinding {
        binding: Binding,
        options: Vec<EnumOption>,
    },
    RequireValueAttribute {
        binding: Binding,
        attribute: Symbol,
        value: Value,
    },
    AttributeBinding {
        binding: Binding,
        attribute: Symbol,
        value_binding: Binding,
    },
    RequireAttribute {
        binding: Binding,
        attribute: Symbol,
    },
    Not {
        body: Vec<CfgOpSelect>,
    },
    Compare {
        operator: CompareOp,
        left: CompareValue,
        right: CompareValue,
    },
    Calculation {
        result_binding: Binding,
        operation: Calculation,
    },
}

#[derive(Debug, Clone)]
pub enum OpenTupleItem {
    Ignore,
    Binding(Binding),
    Compare(Value),
}

#[derive(Debug, Clone)]
pub enum CfgOpApply {
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
        condition: Vec<CfgOpSelect>,
        then_apply: Vec<CfgOpApply>,
        otherwise_apply: Vec<CfgOpApply>,
    },
}
