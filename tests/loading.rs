
use sym_engine::*;

#[test]
fn basics() {

    // single system
    let mut system = System::new("test", &["X"]).unwrap();
    let mut loader = SystemLoader::new(vec![&mut system]);
    loader.load_str("
        rule test:a { $X.x: $x } do { + $X.y: $x }
        rule test:b { $X.x: $x } do { + $X.y: $x }
    ").unwrap();
    assert_eq!(system.count(), 2);

    // multiple systems
    let mut system1 = System::new("test1", &["X"]).unwrap();
    let mut system2 = System::new("test2", &["X"]).unwrap();
    let mut loader = SystemLoader::new(vec![&mut system1, &mut system2]);
    loader.load_str("
        rule test1:a { $X.x: $x } do { + $X.y: $x }
        rule test1:b { $X.x: $x } do { + $X.y: $x }
        rule test2:a { $X.x: $x } do { + $X.y: $x }
    ").unwrap();
    assert_eq!(system1.count(), 2);
    assert_eq!(system2.count(), 1);

    // first matching system counts
    let mut system1 = System::new("test", &["X"]).unwrap();
    let mut system2 = System::new("test", &["X"]).unwrap();
    let mut loader = SystemLoader::new(vec![&mut system1, &mut system2]);
    loader.load_str("
        rule test:a { $X.x: $x } do { + $X.y: $x }
        rule test:b { $X.x: $x } do { + $X.y: $x }
    ").unwrap();
    assert_eq!(system1.count(), 2);
    assert_eq!(system2.count(), 0);
}

#[test]
fn parse_errors() {
    let mut system = System::new("test", &["X"]).unwrap();
    let mut loader = SystemLoader::new(vec![&mut system]);
    assert!(matches!(
        loader.load_str("wrong"),
        Err(LoadError::Parse(_)),
    ));
}

#[test]
fn compile_errors() {
    let mut system = System::new("test", &["X"]).unwrap();
    let mut loader = SystemLoader::new(vec![&mut system]);
    assert!(matches!(
        loader.load_str("rule test:x { $Y.x: $ } do { + $X.x: 23 }"),
        Err(LoadError::Compile(_)),
    ));
}

#[test]
fn duplicate_rule_names() {

    // single load
    let mut system = System::new("test", &["X"]).unwrap();
    let mut loader = SystemLoader::new(vec![&mut system]);
    assert!(matches!(
        loader.load_str("
            rule test:x { $X.x: $ } do { + $X.x: 23 }
            rule test:x { $X.x: $ } do { + $X.x: 23 }
        "),
        Err(LoadError::DuplicateRuleName(sysname, name))
            if name.as_ref() == "x" && sysname.as_ref() == "test",
    ));

    // across loads
    let mut system = System::new("test", &["X"]).unwrap();
    let mut loader = SystemLoader::new(vec![&mut system]);
    loader.load_str("
        rule test:x { $X.x: $ } do { + $X.x: 23 }
    ").unwrap();
    assert!(matches!(
        loader.load_str("
            rule test:x { $X.x: $ } do { + $X.x: 23 }
        "),
        Err(LoadError::DuplicateRuleName(sysname, name))
            if name.as_ref() == "x" && sysname.as_ref() == "test",
    ));
}

#[test]
fn invalid_system() {
    let mut system = System::new("test", &["X"]).unwrap();
    let mut loader = SystemLoader::new(vec![&mut system]);
    assert!(matches!(
        loader.load_str("
            rule test_unknown:x { $X.x: $ } do { + $X.x: 23 }
        "),
        Err(LoadError::NoSuchSystem(name))
            if name.as_ref() == "test_unknown",
    ));
}