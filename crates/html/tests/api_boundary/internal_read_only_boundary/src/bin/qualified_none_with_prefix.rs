fn main() {
    let mut names = html::AtomTable::new();
    let href = names.intern_exact("href").expect("href atom");
    let local = names.resolve_local_name(href).expect("href local name");
    let _invalid = html::QualifiedAttributeName::from_parts(
        html::AttributeNamespace::None,
        Some("xlink"),
        local,
    );
}
