use crate::html5::shared::{
    Attribute, AttributeValue, DocumentParseContext, TextSpan, TextValue, Token,
};
use crate::html5::tokenizer::{TextResolveError, TextResolver, TokenFmt};

#[test]
fn token_fmt_is_deterministic_and_preserves_attribute_order() {
    struct Resolver;
    impl TextResolver for Resolver {
        fn resolve_span(&self, _span: TextSpan) -> Result<&str, TextResolveError> {
            Ok("")
        }
    }

    let mut ctx = DocumentParseContext::new();
    let tag = ctx
        .atoms
        .intern_ascii_folded("div")
        .expect("atom interning");
    let attr_z = ctx
        .atoms
        .intern_ascii_folded("zeta")
        .expect("atom interning");
    let attr_a = ctx
        .atoms
        .intern_ascii_folded("alpha")
        .expect("atom interning");
    let token = Token::StartTag {
        name: tag,
        attrs: vec![
            Attribute {
                name: attr_z,
                value: Some(AttributeValue::Owned("1".to_string())),
            },
            Attribute {
                name: attr_a,
                value: Some(AttributeValue::Owned("2".to_string())),
            },
        ],
        self_closing: false,
    };

    let fmt = TokenFmt::new(&ctx.atoms, &Resolver);
    let first = fmt.format_token(&token).expect("token fmt should succeed");
    let second = fmt.format_token(&token).expect("token fmt should succeed");
    assert_eq!(first, second);
    assert_eq!(
        first,
        "START name=div attrs=[zeta=\"1\" alpha=\"2\"] self_closing=false"
    );
}

#[test]
fn token_fmt_text_is_storage_model_agnostic() {
    struct Resolver;
    impl TextResolver for Resolver {
        fn resolve_span(&self, _span: TextSpan) -> Result<&str, TextResolveError> {
            Ok("hello")
        }
    }

    let span_token = Token::Text {
        text: TextValue::Span(TextSpan::new(0, 0)),
    };
    let owned_token = Token::Text {
        text: TextValue::Owned("hello".to_string()),
    };

    let ctx = DocumentParseContext::new();
    let fmt = TokenFmt::new(&ctx.atoms, &Resolver);
    let span_rendered = fmt
        .format_token(&span_token)
        .expect("span text token fmt should succeed");
    let owned_rendered = fmt
        .format_token(&owned_token)
        .expect("owned text token fmt should succeed");
    assert_eq!(span_rendered, owned_rendered);
    assert_eq!(span_rendered, "CHAR text=\"hello\"");
}

#[test]
fn resolver_rejects_invalid_span() {
    struct Resolver<'a>(&'a str);
    impl<'a> TextResolver for Resolver<'a> {
        fn resolve_span(&self, span: TextSpan) -> Result<&str, TextResolveError> {
            let text = self.0;
            if !(span.start <= span.end
                && span.end <= text.len()
                && text.is_char_boundary(span.start)
                && text.is_char_boundary(span.end))
            {
                return Err(TextResolveError::InvalidSpan { span });
            }
            Ok(&text[span.start..span.end])
        }
    }

    let resolver = Resolver("hi");
    let err = resolver
        .resolve_span(TextSpan::new(0, 999))
        .expect_err("resolver must reject out-of-bounds span");
    assert!(matches!(err, TextResolveError::InvalidSpan { .. }));
}
