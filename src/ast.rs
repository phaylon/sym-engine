
use crate::{Value};
use crate::data::{ArithBinOp, CompareOp};
use crate::parser::{Span};

#[derive(Debug, Clone)]
pub struct Ident<'a> {
    pub span: Span<'a>,
}

impl<'a> Ident<'a> {

    pub fn as_str(&self) -> &str {
        self.span.as_ref()
    }
}

#[derive(Debug, Clone)]
pub struct Path<'a> {
    pub span: Span<'a>,
}

impl<'a> Path<'a> {

    pub fn as_str(&self) -> &str {
        self.span.as_ref()
    }
}

#[derive(Debug, Clone)]
pub enum Variable<'a> {
    Wildcard,
    Ident(Ident<'a>),
}

impl<'a> Variable<'a> {

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Wildcard => None,
            Self::Ident(ident) => Some(ident.as_str()),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Literal<'a> {
    Symbol(Ident<'a>),
    Int(i64),
    Float(f64),
}

impl<'a> Literal<'a> {

    pub fn to_value(&self) -> Value {
        match self {
            Literal::Symbol(ident) => Value::from(ident.as_str()),
            Literal::Int(value) => Value::from(*value),
            Literal::Float(value) => Value::from(*value),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Bindable<'a, T> {
    pub variable: Variable<'a>,
    pub inner: T,
}

#[derive(Debug, Clone)]
pub enum Enumerable<'a> {
    Literal(Literal<'a>),
    Variable(Variable<'a>),
}

#[derive(Debug, Clone)]
pub struct ValueSpec<'a> {
    pub kind: ValueSpecKind<'a>,
    pub position: Span<'a>,
}

#[derive(Debug, Clone)]
pub enum ValueSpecKind<'a> {
    Literal(Literal<'a>),
    Variable(Variable<'a>),
    Tuple(Bindable<'a, Vec<ValueSpec<'a>>>),
    Enum(Bindable<'a, Vec<Enumerable<'a>>>),
    Struct(Bindable<'a, Vec<AttributeSpec<'a>>>),
}

#[derive(Debug, Clone)]
pub struct AttributeSpec<'a> {
    pub position: Span<'a>,
    pub attribute: Ident<'a>,
    pub value_spec: ValueSpec<'a>,
}

#[derive(Debug, Clone)]
pub struct BindingAttributeSpec<'a> {
    pub position: Span<'a>,
    pub variable: Variable<'a>,
    pub attribute_spec: AttributeSpec<'a>,
}

#[derive(Debug, Clone)]
pub struct BindingSpec<'a> {
    pub position: Span<'a>,
    pub variable: Variable<'a>,
    pub value_spec: ValueSpec<'a>,
}

#[derive(Debug, Clone)]
pub enum Comparable<'a> {
    Int(i64),
    Float(f64),
    Variable(Variable<'a>),
}

#[derive(Debug, Clone)]
pub struct Comparison<'a> {
    pub position: Span<'a>,
    pub ordering: CompareOp,
    pub left: Comparable<'a>,
    pub right: Comparable<'a>,
}

#[derive(Debug, Clone)]
pub enum Calculation<'a> {
    Int(i64),
    Float(f64),
    Variable(Variable<'a>),
    BimOp(ArithBinOp, Box<Calculation<'a>>, Box<Calculation<'a>>),
}

#[derive(Debug, Clone)]
pub enum RuleSelect<'a> {
    Binding(BindingSpec<'a>),
    BindingAttribute(BindingAttributeSpec<'a>),
    Comparison(Comparison<'a>),
    Not(Vec<RuleSelect<'a>>),
    Calculation(Variable<'a>, Calculation<'a>, Span<'a>),
}

#[derive(Debug, Clone)]
pub enum RuleApply<'a> {
    Add(BindingAttributeSpec<'a>),
    Remove(BindingAttributeSpec<'a>, bool),
}

#[derive(Debug, Clone)]
pub struct Rule<'a> {
    pub system_name: Path<'a>,
    pub name: Path<'a>,
    pub select: Vec<RuleSelect<'a>>,
    pub apply: Vec<RuleApply<'a>>,
}