use super::{remove_user_prop_prefix, user_prop_key};
#[track_caller]
fn test(input: &str) {
    let as_prop = user_prop_key(input);
    assert_eq!(
        remove_user_prop_prefix(&as_prop),
        Some(input),
        "treated {input:?} as an unprefixed prop"
    );
}

#[test]
fn remove_user_props() {
    assert_eq!(remove_user_prop_prefix("unprefixed"), None);
    assert_eq!(remove_user_prop_prefix(":prop:"), None);
    assert_eq!(remove_user_prop_prefix("sys:"), None);
    assert_eq!(remove_user_prop_prefix("prop:"), Some(""));
    assert_eq!(remove_user_prop_prefix("prop:abc"), Some("abc"));
}

#[test]
fn add_user_props() {
    assert_eq!(user_prop_key("unprefixed"), "prop:unprefixed");
    assert_eq!(user_prop_key(":prop:"), "prop::prop:");
    assert_eq!(user_prop_key("sys:"), "prop:sys:");
    assert_eq!(user_prop_key("prop:"), "prop:prop:");
    assert_eq!(user_prop_key("prop:abc"), "prop:prop:abc");
}

#[test]
fn adds_and_removes() {
    test("akjhwd");
    // can have colons?
    test("prop:");
    // can have colons?
    test(":::");
    // can be empty?
    test("");
}
