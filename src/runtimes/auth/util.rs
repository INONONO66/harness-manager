pub(super) fn to_camel(s: &str) -> String {
    let mut result = String::new();
    let mut cap = false;
    for c in s.chars() {
        if c == '_' {
            cap = true;
        } else if cap {
            result.extend(c.to_uppercase());
            cap = false;
        } else {
            result.push(c);
        }
    }
    result
}
