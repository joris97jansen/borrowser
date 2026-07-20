fn main() {
    let mut names = html::AtomTable::new();
    let lang = names.intern_exact("lang").expect("lang atom");
    let local = names.resolve_local_name(lang).expect("lang local name");
    let _xml = html::QualifiedAttributeName::xml(local);
}
