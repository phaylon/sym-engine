
use std::sync::{Arc};
use crate::{Id};

pub type Symbol = string_cache::DefaultAtom;
pub type Tuple = Arc<[Value]>;

macro_rules! impl_from {
    ($to:ty, $from:ty, $via:expr) => {

        impl From<$from> for $to {
            fn from(value: $from) -> $to { ($via)(value) }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ArithBinOp {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CompareOp {
    Equal,
    NotEqual,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum Value {
    Object(Id),
    Symbol(Symbol),
    Int(i64),
    Float(f64),
    Tuple(Tuple),
}

impl Value {

    pub fn object(&self) -> Option<Id> {
        match *self {
            Self::Object(id) => Some(id),
            _ => None,
        }
    }

    pub fn symbol(&self) -> Option<&Symbol> {
        match *self {
            Self::Symbol(ref symbol) => Some(symbol),
            _ => None,
        }
    }

    pub fn to_symbol(&self) -> Option<Symbol> {
        self.symbol().cloned()
    }

    pub fn into_symbol(self) -> Option<Symbol> {
        match self {
            Self::Symbol(symbol) => Some(symbol),
            _ => None,
        }
    }

    pub fn int(&self) -> Option<i64> {
        match *self {
            Self::Int(value) => Some(value),
            _ => None,
        }
    }

    pub fn float(&self) -> Option<f64> {
        match *self {
            Self::Float(value) => Some(value),
            _ => None,
        }
    }

    pub fn tuple(&self) -> Option<&Tuple> {
        match *self {
            Self::Tuple(ref values) => Some(values),
            _ => None,
        }
    }

    pub fn to_tuple(&self) -> Option<Tuple> {
        self.tuple().cloned()
    }

    pub fn into_tuple(self) -> Option<Tuple> {
        match self {
            Self::Tuple(values) => Some(values),
            _ => None,
        }
    }
}

impl_from!(Value, Id, Value::Object);
impl_from!(Value, Symbol, Value::Symbol);
impl_from!(Value, &str, |value: &str| Value::Symbol(value.into()));
impl_from!(Value, i64, Value::Int);
impl_from!(Value, i32, |value: i32| Value::Int(value.into()));
impl_from!(Value, f64, Value::Float);
impl_from!(Value, f32, |value: f32| Value::Float(value.into()));

impl<T> From<Vec<T>> for Value
where
    T: Into<Value>,
{
    fn from(values: Vec<T>) -> Self {
        Self::Tuple(values.into_iter().map(Into::into).collect())
    }
}

impl std::fmt::Display for Value {

    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Value::Object(id) => std::fmt::Display::fmt(id, fmt),
            Value::Symbol(symbol) => std::fmt::Display::fmt(symbol.as_ref(), fmt),
            Value::Int(value) => std::fmt::Display::fmt(&value, fmt),
            Value::Float(value) => std::fmt::Display::fmt(&value, fmt),
            Value::Tuple(ref values) => {
                write!(fmt, "[")?;
                let mut first = true;
                for value in values.iter() {
                    if first {
                        first = false;
                        write!(fmt, "{}", value)?;
                    } else {
                        write!(fmt, ", {}", value)?;
                    }
                }
                write!(fmt, "]")?;
                Ok(())
            },
        }
    }
}

pub trait MatchValue {

    fn match_value(&self, value: &Value) -> bool;
}

macro_rules! impl_match_value {
    ($matched:ty, $via:expr) => {

        impl MatchValue for $matched {
            fn match_value(&self, value: &Value) -> bool {

                fn hint<F>(f: F) -> F
                where F: FnOnce(&$matched, &Value) -> bool,
                { f }

                (hint($via))(self, value)
            }
        }
    }
}

impl_match_value!(Value, |cmp, val| {
    val == cmp
});

impl_match_value!(Id, |cmp, val| {
    val.object().map(|id| id == *cmp).unwrap_or(false)
});

impl_match_value!(Symbol, |cmp, val| {
    val.symbol().map(|sym| sym == cmp).unwrap_or(false)
});
impl_match_value!(str, |cmp, val| {
    val.symbol().map(|sym| sym.as_ref() == cmp).unwrap_or(false)
});

impl_match_value!(i64, |cmp, val| {
    val.int().map(|val| val == *cmp).unwrap_or(false)
});
impl_match_value!(i32, |cmp, val| {
    val.int().map(|val| val == (*cmp).into()).unwrap_or(false)
});

impl_match_value!(f64, |cmp, val| {
    val.float().map(|val| val == *cmp).unwrap_or(false)
});
impl_match_value!(f32, |cmp, val| {
    val.float().map(|val| val == (*cmp).into()).unwrap_or(false)
});

impl_match_value!(Tuple, |cmp, val| {
    val.tuple().map(|tup| tup == cmp).unwrap_or(false)
});
impl_match_value!([Value], |cmp, val| {
    val.tuple().map(|tup| tup.as_ref() == cmp).unwrap_or(false)
});
