fn main() {
    let _invalid = html::QualifiedAttributeName {
        kind: unreachable!("private state must be inaccessible"),
    };
}
