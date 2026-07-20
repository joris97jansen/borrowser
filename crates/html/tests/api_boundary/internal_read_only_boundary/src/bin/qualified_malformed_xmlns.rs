fn main() {
    let mut names = html::AtomTable::new();
    let xmlns = names.intern_exact("xmlns").expect("xmlns atom");
    let local = names.resolve_local_name(xmlns).expect("xmlns local name");
    let _invalid = html::QualifiedAttributeName::from_parts(
        html::AttributeNamespace::Xmlns,
        Some("xlink"),
        local,
    );
}
