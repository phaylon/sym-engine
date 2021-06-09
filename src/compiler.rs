
use std::sync::{Arc};
use crate::{ast, Value, Symbol};
use crate::data::{ArithBinOp};

mod cfg;
mod optimizer;
mod ops;

pub use ops::{Op, TupleItem};

#[derive(Debug)]
pub struct CompiledRule {
    name: Arc<str>,
    bindings_len: usize,
    ops: Vec<Op>,
    apply_ops: Vec<OpApply>,
}

impl CompiledRule {

    pub fn name(&self) -> &Arc<str> {
        &self.name
    }

    pub fn bindings_len(&self) -> usize {
        self.bindings_len
    }

    pub fn ops(&self) -> &[Op] {
        &self.ops
    }

    pub fn apply_ops(&self) -> &[OpApply] {
        &self.apply_ops
    }
}

#[derive(Debug)]
pub enum CompileError {
    IllegalWildcard {
        line: u32,
    },
    IllegalNamedBinding {
        line: u32,
        name: Arc<str>,
    },
    IllegalBindingMatch {
        line: u32,
        name: Arc<str>,
    },
    RepeatBindings {
        names: Vec<Arc<str>>,
    },
    SingleBindingUse {
        names: Vec<Arc<str>>,
    },
    IllegalReuse {
        line: u32,
        name: Arc<str>,
    },
    IllegalNewBinding {
        line: u32,
        name: Arc<str>,
    },
    IllegalRemoval {
        line: u32,
    },
    IllegalEnumSpecification {
        line: u32,
    },
    IllegalObjectSpecification {
        line: u32,
    },
}

pub fn compile(
    ast: &ast::Rule<'_>,
    input_variables: &[Arc<str>],
) -> Result<CompiledRule, CompileError> {
    let cfg = cfg::ast_to_cfg(ast, input_variables)?;
    let bindings_len = cfg.bindings_len;
    let name = cfg.name.as_ref().into();
    let apply_ops = cfg.apply.clone();
    let ops = optimizer::optimize(cfg, input_variables.len());
    Ok(CompiledRule { name, bindings_len, ops, apply_ops })
}

#[derive(Debug, Clone)]
pub enum Calculation {
    Value(Value),
    Binding(usize),
    BinOp(ArithBinOp, Box<Calculation>, Box<Calculation>),
}

impl Calculation {

    pub fn bindings(&self) -> Vec<usize> {
        let mut bindings = Vec::new();
        self.for_each_binding(&mut |binding| bindings.push(binding));
        bindings
    }

    pub fn for_each_binding<F>(&self, callback: &mut F)
    where
        F: FnMut(usize),
    {
        match *self {
            Calculation::Value(_) => (),
            Calculation::Binding(binding) => callback(binding),
            Calculation::BinOp(_, ref left, ref right) => {
                left.for_each_binding(callback);
                right.for_each_binding(callback);
            },
        }
    }
}

#[derive(Debug, Clone)]
pub enum CompareValue {
    Binding(usize),
    Value(Value),
}

impl CompareValue {

    pub fn to_binding(&self) -> Option<usize> {
        match *self {
            Self::Binding(binding) => Some(binding),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum EnumOption {
    Binding(usize),
    Value(Value),
}

impl EnumOption {

    pub fn to_binding(&self) -> Option<usize> {
        match *self {
            Self::Binding(binding) => Some(binding),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum OpenTupleItem {
    Ignore,
    Binding(usize),
    Compare(Value),
}

#[derive(Debug, Clone)]
pub enum OpApply {
    CreateObject {
        binding: usize,
    },
    CreateTuple {
        binding: usize,
        items: Vec<ApplyTupleItem>,
    },
    AddBindingAttribute {
        binding: usize,
        attribute: Symbol,
        value_binding: usize,
    },
    RemoveBindingAttribute {
        binding: usize,
        attribute: Symbol,
        value_binding: usize,
    },
    AddValueAttribute {
        binding: usize,
        attribute: Symbol,
        value: Value,
    },
    RemoveValueAttribute {
        binding: usize,
        attribute: Symbol,
        value: Value,
    },
}

#[derive(Debug, Clone)]
pub enum ApplyTupleItem {
    Value(Value),
    Binding(usize),
}

#[test]
fn dump() {
    let mut system = crate::System::new("test", &["A"]).unwrap();
    let mut loader = crate::SystemLoader::new(vec![&mut system]);
    loader.load_str("
        rule test:test_a {
            $A.mode: foo,
            $A.a: $a @ {
                x: 23,
                y: [y, $y],
            },
            $A.b: $b @ {
                x: 42,
                y: [y, $y, $x, $x],
            },
            $a != $b,
            not { $a.disable: $ },
        } do {
            - $A.x: 23,
        }
    ").unwrap();
}