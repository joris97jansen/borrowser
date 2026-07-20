fn main() {
    let mut names = html::AtomTable::new();
    let href = names.intern_exact("href").expect("href atom");
    let local = names.resolve_local_name(href).expect("href local name");
    let mut qualified = html::QualifiedAttributeName::unqualified(local);
    qualified.kind = unreachable!("private qualified-name state must not be mutable");
}
