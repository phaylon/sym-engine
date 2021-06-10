
use sym_engine::*;

#[test]
fn from() {

    assert_eq!(Value::from(23i32).int(), Some(23));
    assert_eq!(Value::from(23i64).int(), Some(23));

    assert!(Value::from(23.0f32).float().is_some());
    assert!(Value::from(23.0f64).float().is_some());

    let space = Space::new();
    let id = space.create_id();

    assert_eq!(Value::from(id).object(), Some(id));
    assert_eq!(Value::from("foo").to_symbol(), Some(Symbol::from("foo")));
    assert_eq!(Value::from(Symbol::from("foo")).to_symbol(), Some(Symbol::from("foo")));

    assert_eq!(
        Value::from(vec![Value::from(23), Value::from(42)]),
        Value::Tuple(vec![Value::Int(23), Value::Int(42)].into()),
    );

    assert_eq!(
        Value::from(vec![23, 42]),
        Value::Tuple(vec![Value::Int(23), Value::Int(42)].into()),
    );
}

#[test]
fn match_value() {

    assert!(23.match_value(&Value::Int(23)));
    assert!(!23.match_value(&Value::Int(42)));

    let space = Space::new();
    let id = space.create_id();
    let id2 = space.create_id();

    assert!(id.match_value(&Value::Object(id)));
    assert!(!id.match_value(&Value::Object(id2)));

    assert!("foo".match_value(&Value::Symbol(Symbol::from("foo"))));
    assert!(!"foo".match_value(&Value::Symbol(Symbol::from("bar"))));

    assert!(Symbol::from("foo").match_value(&Value::Symbol(Symbol::from("foo"))));
    assert!(!Symbol::from("foo").match_value(&Value::Symbol(Symbol::from("bar"))));

    let pattern = vec![Value::Int(23), Value::Int(42)];
    assert!(pattern.match_value(&Value::from(vec![23, 42])));
    assert!(!pattern.match_value(&Value::from(vec![42, 23])));

    let pattern = pattern.as_slice();
    assert!(pattern.match_value(&Value::from(vec![23, 42])));
    assert!(!pattern.match_value(&Value::from(vec![42, 23])));
}