
use sym_engine::*;
use assert_matches::{assert_matches};

fn test_run(space: &mut Space, root: Id, rules: &str) -> Option<Value> {
    let mut system = System::new("test", &["ROOT"]).unwrap();
    let mut loader = SystemLoader::new(vec![&mut system]);
    loader.load_str(rules).expect("loaded successfully");
    system.run_to_first(space, &[root]).expect("run successfully");
    space.attributes_mut(root).remove_first_named("result").map(|(_, val)| val)
}

fn load_error(rules: &str) -> Option<LoadError> {
    let mut system = System::new("test", &["ROOT"]).unwrap();
    let mut loader = SystemLoader::new(vec![&mut system]);
    loader.load_str(rules).err()
}

#[test]
fn single_use_error() {
    assert_matches!(
        load_error("rule test:x { $ROOT.x: $x } do {}"),
        Some(LoadError::Compile(CompileError::SingleBindingUse { .. }))
    );
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
        rule test:err {
            $ROOT.deep: { unknown: $ },
        } do {
            + $ROOT.result: wrong,
        }
        rule test:ok {
            $ROOT.deep: { deep_value: $val },
        } do {
            + $ROOT.result: $val,
        }
    "), Some(Value::Int(42)));

    // direct nested literal attribute
    assert_matches!(test_run(&mut space, root, "
        rule test:err {
            $ROOT.deep: { deep_value: 77 },
        } do {
            + $ROOT.result: wrong,
        }
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
        rule test:err {
            $ROOT.deep: $obj,
            $obj: { unknown: $ },
        } do {
            + $ROOT.result: wrong,
        }
        rule test:ok {
            $ROOT.deep: $obj,
            $obj: { deep_value: $value },
        } do {
            + $ROOT.result: $value,
        }
    "), Some(Value::Int(42)));
}

#[test]
fn select_attributes_errors() {

    assert_matches!(
        load_error("rule test:x { $.foo: 23 } do {}"),
        Some(LoadError::Compile(CompileError::IllegalWildcard { .. }))
    );
    assert_matches!(
        load_error("rule test:x { $unknown.foo: 23 } do {}"),
        Some(LoadError::Compile(CompileError::IllegalNewBinding { .. }))
    );
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

    // failure to remove inhibits successful application
    space.attributes_mut(root).add("target", 23);
    assert_matches!(test_run(&mut space, root, "
        rule test:ok {
            $ROOT.target: 23,
        } do {
            + $ROOT.result: 99,
            - $ROOT.target: 123,
        }
    "), None);
    assert!(!space.attributes(root).has_named("value"));

}

#[test]
fn apply_remove_attributes_errors() {

    assert_matches!(
        load_error("rule test:x {} do { - $.value: 23 }"),
        Some(LoadError::Compile(CompileError::IllegalWildcard { .. }))
    );
    assert_matches!(
        load_error("rule test:x {} do { - $unknown.value: 23 }"),
        Some(LoadError::Compile(CompileError::IllegalNewBinding { .. }))
    );
    assert_matches!(
        load_error("rule test:x {} do { - $ROOT.value: $x }"),
        Some(LoadError::Compile(CompileError::IllegalNewBinding { .. }))
    );
    assert_matches!(
        load_error("rule test:x {} do { - $ROOT.value: $ }"),
        Some(LoadError::Compile(CompileError::IllegalWildcard { .. }))
    );
    assert_matches!(
        load_error("rule test:x {} do { - $ROOT.value: {} }"),
        Some(LoadError::Compile(CompileError::IllegalObjectSpecification { .. }))
    );
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
fn apply_add_attributes_errors() {

    assert_matches!(
        load_error("rule test:x {} do { + $.value: 23 }"),
        Some(LoadError::Compile(CompileError::IllegalWildcard { .. }))
    );
    assert_matches!(
        load_error("rule test:x {} do { + $unknown.value: 23 }"),
        Some(LoadError::Compile(CompileError::IllegalNewBinding { .. }))
    );
    assert_matches!(
        load_error("rule test:x {} do { + $ROOT.value: $x }"),
        Some(LoadError::Compile(CompileError::IllegalNewBinding { .. }))
    );
    assert_matches!(
        load_error("rule test:x {} do { + $ROOT.value: $ }"),
        Some(LoadError::Compile(CompileError::IllegalWildcard { .. }))
    );
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
fn select_bindings_errors() {

    assert_matches!(
        load_error("rule test:x { $: 23 } do {}"),
        Some(LoadError::Compile(CompileError::IllegalWildcard { .. }))
    );
    assert_matches!(
        load_error("rule test:x { $unknown: 23 } do {}"),
        Some(LoadError::Compile(CompileError::IllegalNewBinding { .. }))
    );
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

    // capture toplevel
    assert_matches!(test_run(&mut space, root, "
        rule test:ok {
            $ROOT.value: $value,
            $value: x | 23 | y,
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

    // no match
    assert_matches!(test_run(&mut space, root, "
        rule test:ok {
            $ROOT.value: x | 123 | y,
        } do {
            + $ROOT.result: 99,
        }
    "), None);
}

#[test]
fn enum_errors() {

    assert_matches!(
        load_error("rule test:x { $ROOT.x: 23 | $unknown } do {}"),
        Some(LoadError::Compile(CompileError::IllegalNewBinding { .. }))
    );
    assert_matches!(
        load_error("rule test:x { $ROOT.x: 23 | $ } do {}"),
        Some(LoadError::Compile(CompileError::IllegalWildcard { .. }))
    );
    assert_matches!(
        load_error("rule test:x { } do { + $ROOT.x: x | y }"),
        Some(LoadError::Compile(CompileError::IllegalEnumSpecification { .. }))
    );
    assert_matches!(
        load_error("rule test:x { } do { - $ROOT.x: x | y }"),
        Some(LoadError::Compile(CompileError::IllegalEnumSpecification { .. }))
    );
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
    let nest = Value::from(vec![tuple_a.clone(), tuple_c.clone()]);
    let inner = space.create_object().apply(|attrs| {
        attrs.add("inner", 23);
        attrs.object()
    });
    let inner_tuple = Value::from(vec![inner]);
    let root = space.create_object().apply(|attrs| {
        attrs.add("tuple", tuple_a);
        attrs.add("tuple", tuple_b);
        attrs.add("tuple", tuple_c);
        attrs.add("nested", nest);
        attrs.add("with_inner", inner_tuple);
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

    // nesting
    assert_matches!(test_run(&mut space, root, "
        rule test:ok {
            $ROOT.nested: [[foo, $], [$, $value]],
        } do {
            + $ROOT.result: $value,
        }
    "), Some(Value::Int(42)));

    // inner object
    assert_matches!(test_run(&mut space, root, "
        rule test:ok {
            $ROOT.with_inner: [{ inner: $value }],
        } do {
            + $ROOT.result: $value,
        }
    "), Some(Value::Int(23)));
}

#[test]
fn tuple_errors() {

    assert_matches!(
        load_error("rule test:x {} do { + $ROOT.x: [$] }"),
        Some(LoadError::Compile(CompileError::IllegalWildcard { .. }))
    );
    assert_matches!(
        load_error("rule test:x {} do { + $ROOT.x: [$unknown] }"),
        Some(LoadError::Compile(CompileError::IllegalNewBinding { .. }))
    );
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

    // flat
    assert_matches!(test_run(&mut space, root, "
        rule test:ok {
            $ROOT.value: $value,
        } do {
            + $ROOT.result: [foo, $value],
        }
    "), Some(found) if found == tuple);

    // nested
    let nested_tuple = Value::from(vec![
        Value::from("foo"),
        Value::from(vec![
            Value::from("bar"),
            Value::from(23),
        ]),
    ]);
    assert_matches!(test_run(&mut space, root, "
        rule test:ok {} do {
            + $ROOT.result: [foo, [bar, 23]],
        }
    "), Some(found) if found == nested_tuple);

    // nested object
    let nested_object = assert_matches!(test_run(&mut space, root, "
        rule test:ok {} do {
            + $ROOT.result: [{ value: 23 }],
        }
    "), Some(found) => found);
    let nested_object = nested_object
        .tuple().expect("tuple result")[0]
        .object().expect("inner object");
    assert!(space.attributes(nested_object).has("value", &23));
}

#[test]
fn not_clauses() {

    let mut space = Space::new();
    let obj_with_value = space.create_object().apply(|attrs| {
        attrs.add("value", 23);
        attrs.object()
    });
    let obj_without_value = space.create_object().apply(|attrs| {
        attrs.add("other", 42);
        attrs.object()
    });
    let root = space.create_object().apply(|attrs| {
        attrs.add("valued", obj_with_value);
        attrs.add("valued", obj_without_value);
        attrs.add("search", 23);
        attrs.add("other", 23);
        attrs.add("other", 33);
        attrs.object()
    });

    // outer binding
    assert_matches!(test_run(&mut space, root, "
        rule test:ok {
            $ROOT.valued: $obj,
            not { $obj.value: 23 },
        } do {
            + $ROOT.result: $obj,
        }
    "), Some(Value::Object(id)) if id == obj_without_value);

    // inner bindings
    assert_matches!(test_run(&mut space, root, "
        rule test:ok {
            $ROOT.other: $value,
            not {
                $ROOT.valued: { value: $value },
            },
        } do {
            + $ROOT.result: $value,
        }
    "), Some(Value::Int(33)));

    // no match
    assert_matches!(test_run(&mut space, root, "
        rule test:ok {
            not { $ROOT.search: $ },
        } do {
            + $ROOT.result: wrong,
        }
    "), None);
}

#[test]
fn not_clauses_errors() {

    assert_matches!(
        load_error("rule test:x { not { $.x: 23 } } do {}"),
        Some(LoadError::Compile(CompileError::IllegalWildcard { .. }))
    );
    assert_matches!(
        load_error("rule test:x { not { $ROOT.value: $x }, $ROOT.other: $x } do {}"),
        Some(LoadError::Compile(CompileError::RepeatBindings { .. }))
    );
}

#[test]
fn math() {

    let mut space = Space::new();
    let root = space.create_id();
    space.attributes_mut(root).add("value", 23);

    // add
    assert_matches!(test_run(&mut space, root, "
        rule test:ok {
            $ROOT.value: $value,
            $out is $value + 10,
        } do {
            + $ROOT.result: $out,
        }
    "), Some(Value::Int(33)));

    // subtract
    assert_matches!(test_run(&mut space, root, "
        rule test:ok {
            $ROOT.value: $value,
            $out is $value - 10,
        } do {
            + $ROOT.result: $out,
        }
    "), Some(Value::Int(13)));

    // multiply
    assert_matches!(test_run(&mut space, root, "
        rule test:ok {
            $ROOT.value: $value,
            $out is $value * 10,
        } do {
            + $ROOT.result: $out,
        }
    "), Some(Value::Int(230)));

    // division
    assert_matches!(test_run(&mut space, root, "
        rule test:ok {
            $ROOT.value: $value,
            $out is 46 / $value,
        } do {
            + $ROOT.result: $out,
        }
    "), Some(Value::Int(2)));

    // multiply before add
    assert_matches!(test_run(&mut space, root, "
        rule test:ok {
            $out is 2*3+4*5,
        } do {
            + $ROOT.result: $out,
        }
    "), Some(Value::Int(26)));

    // grouping
    assert_matches!(test_run(&mut space, root, "
        rule test:ok {
            $out is 2*(3+4)*5,
        } do {
            + $ROOT.result: $out,
        }
    "), Some(Value::Int(70)));
}

#[test]
fn math_errors() {

    assert_matches!(
        load_error("rule test:x { $ is 2+3 } do {}"),
        Some(LoadError::Compile(CompileError::IllegalWildcard { .. }))
    );
    assert_matches!(
        load_error("rule test:x { $ROOT.x: $x, $x is 2+3 } do {}"),
        Some(LoadError::Compile(CompileError::IllegalReuse { .. }))
    );
    assert_matches!(
        load_error("rule test:x { $new is 2+$ } do {}"),
        Some(LoadError::Compile(CompileError::IllegalWildcard { .. }))
    );
    assert_matches!(
        load_error("rule test:x { $new is 2+$unknown } do {}"),
        Some(LoadError::Compile(CompileError::IllegalNewBinding { .. }))
    );
}

#[test]
fn comparisons() {

    let mut space = Space::new();
    let root = space.create_id();
    space.attributes_mut(root).add("value", 5);
    space.attributes_mut(root).add("value", 10);
    space.attributes_mut(root).add("value", 15);

    // equals
    assert_matches!(test_run(&mut space, root, "
        rule test:ok {
            $ROOT.value: $value,
            $value == 10,
        } do {
            + $ROOT.result: $value,
        }
    "), Some(Value::Int(10)));

    // not equals
    assert_matches!(test_run(&mut space, root, "
        rule test:ok {
            $ROOT.value: $value,
            $value != 5,
        } do {
            + $ROOT.result: $value,
        }
    "), Some(Value::Int(10)));

    // less
    assert_matches!(test_run(&mut space, root, "
        rule test:ok {
            $ROOT.value: $value,
            $value < 15,
        } do {
            + $ROOT.result: $value,
        }
    "), Some(Value::Int(5)));

    // greater
    assert_matches!(test_run(&mut space, root, "
        rule test:ok {
            $ROOT.value: $value,
            $value > 5,
        } do {
            + $ROOT.result: $value,
        }
    "), Some(Value::Int(10)));

    // less or equal
    assert_matches!(test_run(&mut space, root, "
        rule test:ok {
            $ROOT.value: $value,
            $value <= 5,
        } do {
            + $ROOT.result: $value,
        }
    "), Some(Value::Int(5)));
    assert_matches!(test_run(&mut space, root, "
        rule test:ok {
            $ROOT.value: $value,
            $value <= 10,
        } do {
            + $ROOT.result: $value,
        }
    "), Some(Value::Int(5)));

    // greater or equal
    assert_matches!(test_run(&mut space, root, "
        rule test:ok {
            $ROOT.value: $value,
            $value >= 5,
        } do {
            + $ROOT.result: $value,
        }
    "), Some(Value::Int(5)));
    assert_matches!(test_run(&mut space, root, "
        rule test:ok {
            $ROOT.value: $value,
            $value >= 10,
        } do {
            + $ROOT.result: $value,
        }
    "), Some(Value::Int(10)));
}

#[test]
fn comparison_errors() {

    assert_matches!(
        load_error("rule test:x { $ROOT == $ } do {}"),
        Some(LoadError::Compile(CompileError::IllegalWildcard { .. }))
    );
    assert_matches!(
        load_error("rule test:x { $ROOT == $unknown } do {}"),
        Some(LoadError::Compile(CompileError::IllegalNewBinding { .. }))
    );
}