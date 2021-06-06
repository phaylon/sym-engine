
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

macro_rules! fns_variant {
    (
        $value:ty, $variant:ident, $output:ty,
        $is_variant:ident,
        $as_variant:ident,
        $to_variant:ident,
        $into_variant:ident
    ) => {

        pub fn $is_variant(&self) -> bool {
            match *self {
                Self::$variant(_) => true,
                _ => false,
            }
        }

        pub fn $as_variant(&self) -> Option<&$output> {
            match *self {
                Self::$variant(ref value) => Some(value),
                _ => None,
            }
        }

        pub fn $to_variant(&self) -> Option<$output> {
            match *self {
                Self::$variant(ref value) => Some(value.clone()),
                _ => None,
            }
        }

        pub fn $into_variant(self) -> Option<$output> {
            match self {
                Self::$variant(value) => Some(value),
                _ => None,
            }
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

    fns_variant!(Value, Object, Id, is_object, as_object, to_object, into_object);
    fns_variant!(Value, Symbol, Symbol, is_symbol, as_symbol, to_symbol, into_symbol);
    fns_variant!(Value, Int, i64, is_int, as_int, to_int, into_int);
    fns_variant!(Value, Float, f64, is_float, as_float, to_float, into_float);
    fns_variant!(Value, Tuple, Tuple, is_tuple, as_tuple, to_tuple, into_tuple);
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

impl_match_value!(Id, |cmp, val| {
    val.to_object().map(|id| id == *cmp).unwrap_or(false)
});

impl_match_value!(Symbol, |cmp, val| {
    val.as_symbol().map(|sym| sym == cmp).unwrap_or(false)
});
impl_match_value!(str, |cmp, val| {
    val.as_symbol().map(|sym| sym.as_ref() == cmp).unwrap_or(false)
});

impl_match_value!(i64, |cmp, val| {
    val.to_int().map(|val| val == *cmp).unwrap_or(false)
});
impl_match_value!(i32, |cmp, val| {
    val.to_int().map(|val| val == (*cmp).into()).unwrap_or(false)
});

impl_match_value!(f64, |cmp, val| {
    val.to_float().map(|val| val == *cmp).unwrap_or(false)
});
impl_match_value!(f32, |cmp, val| {
    val.to_float().map(|val| val == (*cmp).into()).unwrap_or(false)
});

impl_match_value!(Tuple, |cmp, val| {
    val.as_tuple().map(|tup| tup == cmp).unwrap_or(false)
});
impl_match_value!([Value], |cmp, val| {
    val.as_tuple().map(|tup| tup.as_ref() == cmp).unwrap_or(false)
});
