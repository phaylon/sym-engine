
use sym_engine::*;
use assert_matches::{assert_matches};

fn test_run(space: &mut Space, root: Id, rules: &str) -> Option<Value> {
    let mut system = System::new("test", &["ROOT"]).unwrap();
    let mut loader = SystemLoader::new(vec![&mut system]);
    loader.load_str(rules).expect("loaded successfully");
    system.run_to_first(space, &[root]).expect("run successfully");
    space.attributes_mut(root).remove_first_named("result").map(|(_, val)| val)
}

#[test]
fn select_attributes() {

    let mut space = Space::new();
    let deep = space.create_object().apply(|attrs| {
        attrs.add("deep_value", 42);
        attrs.object()
    });
    let deep_wrong = space.create_object().apply(|attrs| {
        attrs.add("wrong", 99);
        attrs.object()
    });
    let root = space.create_object().apply(|attrs| {
        attrs.add("value", 23);
        attrs.add("deep", deep_wrong);
        attrs.add("deep", deep);
        attrs.object()
    });

    // variable attributes
    assert_matches!(test_run(&mut space, root, "
        rule test:no { $ROOT.other: $ } do { + $ROOT.result: wrong }
        rule test:ok { $ROOT.value: $v } do { + $ROOT.result: $v }
    "), Some(Value::Int(23)));

    // literal attributes
    assert_matches!(test_run(&mut space, root, "
        rule test:no { $ROOT.value: 42 } do { + $ROOT.result: wrong }
        rule test:ok { $ROOT.value: 23 } do { + $ROOT.result: 42 }
    "), Some(Value::Int(42)));

    // direct nested variable attribute
    assert_matches!(test_run(&mut space, root, "
        rule test:ok {
            $ROOT.deep: { deep_value: $val },
        } do {
            + $ROOT.result: $val,
        }
    "), Some(Value::Int(42)));

    // direct nested literal attribute
    assert_matches!(test_run(&mut space, root, "
        rule test:ok {
            $ROOT.deep: { deep_value: 42 },
        } do {
            + $ROOT.result: 42,
        }
    "), Some(Value::Int(42)));

    // capture object
    assert_matches!(test_run(&mut space, root, "
        rule test:ok {
            $ROOT.deep: $obj @ { deep_value: 42 },
        } do {
            + $ROOT.result: $obj,
        }
    "), Some(Value::Object(id)) if id == deep);

    // indirect nested attributes
    assert_matches!(test_run(&mut space, root, "
        rule test:ok {
            $ROOT.deep: $obj,
            $obj.deep_value: $value,
        } do {
            + $ROOT.result: $value,
        }
    "), Some(Value::Int(42)));

    // toplevel binding object
    assert_matches!(test_run(&mut space, root, "
        rule test:ok {
            $ROOT.deep: $obj,
            $obj: { deep_value: $value },
        } do {
            + $ROOT.result: $value,
        }
    "), Some(Value::Int(42)));
}

#[test]
fn apply_remove_attributes() {

    let mut space = Space::new();
    let root = space.create_id();

    // variables
    space.attributes_mut(root).add("value", 23);
    assert_matches!(test_run(&mut space, root, "
        rule test:ok {
            $ROOT.value: $value,
        } do {
            + $ROOT.result: $value,
            - $ROOT.value: $value,
        }
    "), Some(Value::Int(23)));
    assert!(!space.attributes(root).has_named("value"));

    // literals
    space.attributes_mut(root).add("value", 23);
    assert_matches!(test_run(&mut space, root, "
        rule test:ok {
            $ROOT.value: 23,
        } do {
            + $ROOT.result: 99,
            - $ROOT.value: 23,
        }
    "), Some(Value::Int(99)));
    assert!(!space.attributes(root).has_named("value"));
}

#[test]
fn apply_add_attributes() {

    let mut space = Space::new();
    let root = space.create_id();
    space.attributes_mut(root).add("input", 23);

    // literals
    assert_matches!(test_run(&mut space, root, "
        rule test:ok {} do {
            + $ROOT.value: 23,
            + $ROOT.result: 99,
        }
    "), Some(Value::Int(99)));
    assert_matches!(
        space.attributes_mut(root).remove_first_named("value"),
        Some((_, Value::Int(23)))
    );
    assert!(!space.attributes(root).has_named("value"));

    // variables
    assert_matches!(test_run(&mut space, root, "
        rule test:ok { $ROOT.input: $value } do {
            + $ROOT.result: $value,
        }
    "), Some(Value::Int(23)));

    // nested
    assert_matches!(test_run(&mut space, root, "
        rule test:ok {} do {
            + $ROOT.result: 23,
            + $ROOT.nested: { x: 2, x: 3 },
        }
    "), Some(Value::Int(23)));
    let (_, value) = space.attributes_mut(root).remove_first_named("nested").unwrap();
    let nested = value.object().unwrap();
    assert!(space.attributes(nested).has("x", &2));
    assert!(space.attributes(nested).has("x", &3));

    // nested with capture
    assert_matches!(test_run(&mut space, root, "
        rule test:ok {} do {
            + $ROOT.result: 23,
            + $ROOT.nested: $new @ { x: 2, x: 3 },
            + $ROOT.new: $new,
        }
    "), Some(Value::Int(23)));
    let (_, value) = space.attributes_mut(root).remove_first_named("nested").unwrap();
    let nested = value.object().unwrap();
    assert!(space.attributes(nested).has("x", &2));
    assert!(space.attributes(nested).has("x", &3));
    let (_, value) = space.attributes_mut(root).remove_first_named("new").unwrap();
    let new = value.object().unwrap();
    assert_eq!(nested, new);
}

#[test]
fn select_bindings() {

    let mut space = Space::new();
    let root = space.create_object().apply(|attrs| {
        attrs.add("value", "foo");
        attrs.object()
    });

    // literal
    assert_matches!(test_run(&mut space, root, "
        rule test:err {
            $ROOT.value: $value,
            $value: bar,
        } do {
            + $ROOT.result: wrong,
        }
        rule test:ok {
            $ROOT.value: $value,
            $value: foo,
        } do {
            + $ROOT.result: found,
        }
    "), Some(Value::Symbol(symbol)) if symbol.as_ref() == "found");
}

#[test]
fn enums() {

    let mut space = Space::new();
    let deep = space.create_object().apply(|attrs| {
        attrs.add("deep_value", 42);
        attrs.object()
    });
    let root = space.create_object().apply(|attrs| {
        attrs.add("value", 23);
        attrs.add("spec", 23);
        attrs.add("deep", deep);
        attrs.object()
    });

    // capture
    assert_matches!(test_run(&mut space, root, "
        rule test:ok {
            $ROOT.value: $value @ x | 42 | 23 | 99,
        } do {
            + $ROOT.result: $value,
        }
    "), Some(Value::Int(23)));

    // match
    assert_matches!(test_run(&mut space, root, "
        rule test:ok {
            $ROOT.spec: $spec,
            $ROOT.value: $value @ x | 42 | $spec | 99,
        } do {
            + $ROOT.result: $value,
        }
    "), Some(Value::Int(23)));

    // attributes
    assert_matches!(test_run(&mut space, root, "
        rule test:ok {
            $ROOT.deep: {
                deep_value: $value @ x | 42 | 23 | y,
            },
        } do {
            + $ROOT.result: $value,
        }
    "), Some(Value::Int(42)));

    // no capture
    assert_matches!(test_run(&mut space, root, "
        rule test:ok {
            $ROOT.value: x | 23 | y,
        } do {
            + $ROOT.result: 99,
        }
    "), Some(Value::Int(99)));
}

#[test]
fn wildcards() {

    let mut space = Space::new();
    let attr_ok = space.create_object().apply(|attrs| {
        attrs.add("value", 23);
        attrs.add("mark", 99);
        attrs.object()
    });
    let attr_err = space.create_object().apply(|attrs| {
        attrs.add("value", 42);
        attrs.object()
    });
    let root = space.create_object().apply(|attrs| {
        attrs.add("attr", attr_err);
        attrs.add("attr", attr_ok);
        attrs.object()
    });

    // attributes
    assert_matches!(test_run(&mut space, root, "
        rule test:ok {
            $ROOT.attr: {
                value: $value,
                mark: $,
            },
        } do {
            + $ROOT.result: $value,
        }
    "), Some(Value::Int(23)));
}

#[test]
fn select_tuples() {

    let mut space = Space::new();
    let tuple_a = Value::from(vec![Value::from("foo"), Value::from(13)]);
    let tuple_b = Value::from(vec![Value::from("foo"), Value::from(23), Value::from(42)]);
    let tuple_c = Value::from(vec![Value::from("bar"), Value::from(42)]);
    let root = space.create_object().apply(|attrs| {
        attrs.add("tuple", tuple_a);
        attrs.add("tuple", tuple_b);
        attrs.add("tuple", tuple_c);
        attrs.object()
    });

    // matches tuple/3
    assert_matches!(test_run(&mut space, root, "
        rule test:ok {
            $ROOT.tuple: [foo, $value, 42],
        } do {
            + $ROOT.result: $value,
        }
    "), Some(Value::Int(23)));

    // skip to correct tuple
    assert_matches!(test_run(&mut space, root, "
        rule test:ok {
            $ROOT.tuple: [bar, $value],
        } do {
            + $ROOT.result: $value,
        }
    "), Some(Value::Int(42)));

    // wildcard
    assert_matches!(test_run(&mut space, root, "
        rule test:ok {
            $ROOT.tuple: [$, $value],
        } do {
            + $ROOT.result: $value,
        }
    "), Some(Value::Int(13)));
}

#[test]
fn apply_remove_tuple() {

    let mut space = Space::new();
    let tuple = Value::from(vec![Value::from("foo"), Value::from(23)]);
    let root = space.create_id();

    // remove by structure
    space.attributes_mut(root).add("tuple", tuple.clone());
    assert_matches!(test_run(&mut space, root, "
        rule test:ok {
            $ROOT.tuple: [foo, $value],
        } do {
            + $ROOT.result: $value,
            - $ROOT.tuple: [foo, $value],
        }
    "), Some(Value::Int(23)));
    assert!(!space.attributes(root).has_named("tuple"));

    // remove by value
    space.attributes_mut(root).add("tuple", tuple.clone());
    assert_matches!(test_run(&mut space, root, "
        rule test:ok {
            $ROOT.tuple: $found @ [foo, $value],
        } do {
            + $ROOT.result: $value,
            - $ROOT.tuple: $found,
        }
    "), Some(Value::Int(23)));
    assert!(!space.attributes(root).has_named("tuple"));
}

#[test]
fn apply_add_tuple() {

    let mut space = Space::new();
    let tuple = Value::from(vec![Value::from("foo"), Value::from(23)]);
    let root = space.create_id();
    space.attributes_mut(root).add("value", 23);

    assert_matches!(test_run(&mut space, root, "
    rule test:ok {
        $ROOT.value: $value,
    } do {
        + $ROOT.result: [foo, $value],
    }
    "), Some(found) if found == tuple);
}