
use std::sync::{Arc};
use std::cell::{RefCell};
use num_traits::{ToPrimitive};
use crate::{ast, Value, Symbol};
use crate::data::{ArithBinOp};

mod cfg;
mod optimizer;
mod ops;
mod builder;

pub use ops::{Op, TupleItem};
pub use builder::{
    SelectBuilder,
    EnumBuilder,
    TupleBuilder,
    BuilderBinding,
    CalcBuilder,
    CalcBuilderNode,
    CompareBuilder,
    ApplyBuilder,
    ApplyTupleBuilder,
};

#[derive(Debug, Clone)]
struct BindingSequence {
    next: RefCell<u16>,
}

impl BindingSequence {

    pub fn new() -> Self {
        Self {
            next: RefCell::new(0),
        }
    }

    pub fn len(&self) -> usize {
        let next: u16 = *self.next.borrow();
        next as usize
    }

    pub fn next(&self) -> Binding {
        let mut counter = self.next.borrow_mut();
        let index = *counter;
        *counter = counter.checked_add(1).expect("exceeded maximum binding count");
        Binding(index)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Binding(u16);

impl Binding {

    pub fn with_index(index: usize) -> Self {
        let index: u16 = index.to_u16().expect("raw binding index within range");
        Binding(index)
    }

    pub fn index(&self) -> usize {
        self.0 as usize
    }
}

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

struct DisplayVarNames<'a>(&'a [Arc<str>]);

impl std::fmt::Display for DisplayVarNames<'_> {

    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        let mut first = true;
        for var in self.0 {
            if first {
                first = false;
                write!(fmt, "`${}`", var)?;
            } else {
                write!(fmt, ", `${}`", var)?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum CompileError {
    #[error("illegal wildcard variable at line {line}")]
    IllegalWildcard {
        line: u32,
    },
    #[error("illegal named binding `${name}` at line {line}")]
    IllegalNamedBinding {
        line: u32,
        name: Arc<str>,
    },
    #[error("illegal match against binding `${name}` at line {line}")]
    IllegalBindingMatch {
        line: u32,
        name: Arc<str>,
    },
    #[error("multiple distinct bindings to {} are generated", DisplayVarNames(.names))]
    RepeatBindings {
        names: Vec<Arc<str>>,
    },
    #[error("bindings {} are only used once", DisplayVarNames(.names))]
    SingleBindingUse {
        names: Vec<Arc<str>>,
    },
    #[error("illegal reuse of variable `${name}` for binding at line {line}")]
    IllegalReuse {
        line: u32,
        name: Arc<str>,
    },
    #[error("existing binding required instead of `${name}` at line {line}")]
    ExistingBindingRequired {
        line: u32,
        name: Arc<str>,
    },
    #[error("illegal removal speecification at line {line}")]
    IllegalRemoval {
        line: u32,
    },
    #[error("illegal place for enum specification at line {line}")]
    IllegalEnumSpecification {
        line: u32,
    },
    #[error("illegal place for object specification at line {line}")]
    IllegalObjectSpecification {
        line: u32,
    },
}

pub fn build_and_compile<F>(
    name: Arc<str>,
    input_variables: &[Arc<str>],
    builder_cb: F,
) -> CompiledRule
where
    F: for<'seq, 'bind> FnOnce(
        SelectBuilder<'seq, 'bind>,
        &[BuilderBinding<'bind>],
    ) -> ApplyBuilder<'seq, 'bind>,
{
    let builder::BuiltRule { select, apply, bindings_len }
        = builder::build(input_variables.len(), builder_cb);
    let cfg = cfg::CfgRule { name, select, apply, bindings_len };
    compile_cfg(cfg, input_variables.len())
}

pub fn compile(
    ast: &ast::Rule<'_>,
    input_variables: &[Arc<str>],
) -> Result<CompiledRule, CompileError> {
    let cfg = cfg::ast_to_cfg(ast, input_variables)?;
    Ok(compile_cfg(cfg, input_variables.len()))
}

fn compile_cfg(
    cfg: cfg::CfgRule,
    input_variables_len: usize,
) -> CompiledRule {
    let bindings_len = cfg.bindings_len;
    let name = cfg.name.as_ref().into();
    let apply_ops = cfg.apply.clone();
    let ops = optimizer::optimize(cfg, input_variables_len);
    CompiledRule { name, bindings_len, ops, apply_ops }
}

#[derive(Debug, Clone)]
pub enum Calculation {
    Value(Value),
    Binding(Binding),
    BinOp(ArithBinOp, Box<Calculation>, Box<Calculation>),
}

impl Calculation {

    pub fn bindings(&self) -> Vec<Binding> {
        let mut bindings = Vec::new();
        self.for_each_binding(&mut |binding| bindings.push(binding));
        bindings
    }

    pub fn for_each_binding<F>(&self, callback: &mut F)
    where
        F: FnMut(Binding),
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
    Binding(Binding),
    Value(Value),
}

impl CompareValue {

    pub fn to_binding(&self) -> Option<Binding> {
        match *self {
            Self::Binding(binding) => Some(binding),
            _ => None,
        }
    }

    pub fn resolve<'a>(&'a self, bindings: &'a [Value]) -> &'a Value {
        match self {
            CompareValue::Binding(binding) => &bindings[binding.index()],
            CompareValue::Value(value) => value,
        }
    }
}

#[derive(Debug, Clone)]
pub enum EnumOption {
    Binding(Binding),
    Value(Value),
}

impl EnumOption {

    pub fn to_binding(&self) -> Option<Binding> {
        match *self {
            Self::Binding(binding) => Some(binding),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum OpenTupleItem {
    Ignore,
    Binding(Binding),
    Compare(Value),
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
    },
}

#[derive(Debug, Clone)]
pub enum ApplyTupleItem {
    Value(Value),
    Binding(Binding),
}
