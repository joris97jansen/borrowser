use super::super::Html5ParseSession;
#[cfg(feature = "parser-conformance")]
use crate::html5::shared::ParserObservationConfig;
use crate::html5::shared::{
    DocumentParseContext, ErrorPolicy, Html5SessionError, ParserFailureInjection, ParserFatalError,
    ParserReservationSite,
};
use crate::html5::tokenizer::TokenizerConfig;
use crate::html5::tree_builder::TreeBuilderConfig;
use std::num::NonZeroU64;

fn injection(site: ParserReservationSite) -> ParserFailureInjection {
    ParserFailureInjection::new(site, NonZeroU64::MIN)
}

fn assert_site(error: Html5SessionError, site: ParserReservationSite) {
    assert!(matches!(
        error,
        Html5SessionError::Fatal(ParserFatalError::ResourceExhaustion(exhaustion))
            if exhaustion.site() == site
    ));
}

#[test]
fn construction_reservation_failure_is_returned_without_a_session() {
    for site in [
        ParserReservationSite::KnownTagAtomStorage,
        ParserReservationSite::KnownTagLookupStorage,
    ] {
        let ctx =
            DocumentParseContext::with_failure_injection(ErrorPolicy::default(), injection(site));
        let error = match Html5ParseSession::new(
            TokenizerConfig::default(),
            TreeBuilderConfig::default(),
            ctx,
        ) {
            Ok(_) => panic!("known-tag reservation failure must abort construction"),
            Err(error) => error,
        };
        assert_site(error, site);
    }
}

#[test]
#[cfg(feature = "parser-conformance")]
fn live_fatal_failure_is_latched_across_mutation_finish_and_drains() {
    let ctx = DocumentParseContext::with_observations_and_failure_injection(
        ErrorPolicy::default(),
        ParserObservationConfig::default(),
        injection(ParserReservationSite::TemplateChildStorage),
    );
    let mut session = Html5ParseSession::new(
        TokenizerConfig::default(),
        TreeBuilderConfig::default(),
        ctx,
    )
    .expect("session construction");

    session
        .push_str("<template>")
        .expect("input append before fatal");
    let first = session.pump().expect_err("template reservation must fail");
    assert_site(first, ParserReservationSite::TemplateChildStorage);

    for _ in 0..2 {
        for error in [
            session
                .push_bytes(b"x")
                .expect_err("push_bytes after fatal"),
            session.push_str("x").expect_err("push_str after fatal"),
            session.pump().expect_err("pump after fatal"),
            session.finish().expect_err("finish after fatal"),
            session.take_patches().expect_err("patch drain after fatal"),
            session
                .take_patch_batch()
                .expect_err("batch drain after fatal"),
            session
                .take_observations_for_conformance()
                .expect_err("observation drain after fatal"),
        ] {
            assert_eq!(error, first);
        }
    }

    assert!(
        session.parse_errors().iter().all(|error| {
            !matches!(
                error.code,
                crate::html5::shared::LegacyParseErrorCode::ResourceLimit
            )
        }),
        "fatal reservation failure must not be recorded as an authored parse error or configured limit"
    );
}

#[test]
fn parser_local_injection_is_deterministic_under_parallel_sessions() {
    let workers: Vec<_> = (0..4)
        .map(|_| {
            std::thread::spawn(|| {
                let ctx = DocumentParseContext::with_failure_injection(
                    ErrorPolicy::default(),
                    injection(ParserReservationSite::TemplateChildStorage),
                );
                let mut session = Html5ParseSession::new(
                    TokenizerConfig::default(),
                    TreeBuilderConfig::default(),
                    ctx,
                )
                .expect("session construction");
                session.push_str("<template>").expect("template input");
                session.pump().expect_err("parallel injected failure")
            })
        })
        .collect();

    for worker in workers {
        assert_site(
            worker.join().expect("parallel parser thread"),
            ParserReservationSite::TemplateChildStorage,
        );
    }
}
