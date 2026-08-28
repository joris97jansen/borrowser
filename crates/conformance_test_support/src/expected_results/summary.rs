use std::collections::BTreeMap;
use std::fmt::Write;

use super::model::*;

pub fn serialize_expected_results_summary(results: &ValidatedExpectedResults) -> Vec<u8> {
    let discovered = results.records().len();
    let classified = results
        .records()
        .iter()
        .filter(|record| matches!(record.classification(), Classification::Classified(_)))
        .count();
    let mut output = String::new();
    write_string(&mut output, "format", EXPECTED_RESULTS_SUMMARY_FORMAT_V1);
    write_string(&mut output, "granularity", EXPECTED_RESULTS_GRANULARITY_V1);
    write_count(&mut output, "discovered", discovered);

    section(&mut output, "classification");
    write_count(&mut output, "population", discovered);
    write_count(&mut output, "classified", classified);
    write_count(&mut output, "not_yet_classified", discovered - classified);

    let classified_records = results.records().iter().filter_map(|record| {
        let Classification::Classified(metadata) = record.classification() else {
            return None;
        };
        Some(metadata)
    });

    let mut engine_available = 0;
    let mut engine_unavailable = 0;
    let mut engine_unknown = 0;
    let mut harness_ready = 0;
    let mut harness_not_ready = 0;
    let mut harness_unknown = 0;
    let mut expected_pass = 0;
    let mut expected_fail = 0;
    let mut stable = 0;
    let mut flaky = 0;
    let mut stability_unknown = 0;
    let mut lane_counts = BTreeMap::<LanePolicyScope, usize>::new();
    let mut missing_capability_counts = BTreeMap::<EngineCapabilityKind, usize>::new();
    let mut harness_limitation_counts = BTreeMap::<HarnessLimitationKind, usize>::new();
    let mut environment_kind_counts = BTreeMap::<EnvironmentRequirementKind, usize>::new();
    let mut environment_profiles = BTreeMap::<(EnvironmentRequirementKind, String), usize>::new();
    let mut tests_with_environment_requirements = 0;
    let mut requirement_counts = BTreeMap::<RequirementTag, usize>::new();

    for metadata in classified_records {
        match metadata.engine() {
            EngineCapabilityAvailability::Available => engine_available += 1,
            EngineCapabilityAvailability::Unavailable { missing } => {
                engine_unavailable += 1;
                for capability in missing {
                    *missing_capability_counts
                        .entry(capability.kind())
                        .or_default() += 1;
                }
            }
            EngineCapabilityAvailability::NotYetEstablished => engine_unknown += 1,
        }
        match metadata.harness() {
            HarnessReadiness::Ready => harness_ready += 1,
            HarnessReadiness::NotReady { limitations } => {
                harness_not_ready += 1;
                for limitation in limitations {
                    *harness_limitation_counts
                        .entry(limitation.kind())
                        .or_default() += 1;
                }
            }
            HarnessReadiness::NotYetEstablished => harness_unknown += 1,
        }
        match metadata.expectation() {
            Expectation::ExpectedPass => expected_pass += 1,
            Expectation::ExpectedFail { .. } => expected_fail += 1,
        }
        match metadata.stability() {
            Stability::Stable => stable += 1,
            Stability::Flaky { .. } => flaky += 1,
            Stability::NotYetEstablished => stability_unknown += 1,
        }
        for exclusion in metadata.lane_exclusions() {
            *lane_counts.entry(exclusion.policy()).or_default() += 1;
        }
        if !metadata.environment().requirements().is_empty() {
            tests_with_environment_requirements += 1;
        }
        for requirement in metadata.environment().requirements() {
            *environment_kind_counts
                .entry(requirement.key().kind())
                .or_default() += 1;
            *environment_profiles
                .entry((
                    requirement.key().kind(),
                    requirement.key().profile().as_str().to_owned(),
                ))
                .or_default() += 1;
        }
        for requirement in metadata.requirements() {
            *requirement_counts.entry(*requirement).or_default() += 1;
        }
    }

    section(&mut output, "engine_capability");
    write_count(&mut output, "population", classified);
    write_count(&mut output, "available", engine_available);
    write_count(&mut output, "unavailable", engine_unavailable);
    write_count(&mut output, "not_yet_established", engine_unknown);

    section(&mut output, "harness_readiness");
    write_count(&mut output, "population", classified);
    write_count(&mut output, "ready", harness_ready);
    write_count(&mut output, "not_ready", harness_not_ready);
    write_count(&mut output, "not_yet_established", harness_unknown);

    section(&mut output, "expectation");
    write_count(&mut output, "population", classified);
    write_count(&mut output, "expected_pass", expected_pass);
    write_count(&mut output, "expected_fail", expected_fail);

    section(&mut output, "expected_failure_classes");
    write_count(&mut output, "semantic_mismatch", expected_fail);

    section(&mut output, "stability");
    write_count(&mut output, "population", classified);
    write_count(&mut output, "stable", stable);
    write_count(&mut output, "flaky", flaky);
    write_count(&mut output, "not_yet_established", stability_unknown);

    section(&mut output, "lane_exclusion_declarations");
    write_count(
        &mut output,
        "declarations",
        lane_counts.values().copied().sum(),
    );
    for policy in LanePolicyScope::ALL {
        write_count(
            &mut output,
            &summary_key(policy.as_str()),
            lane_counts.get(&policy).copied().unwrap_or(0),
        );
    }

    section(&mut output, "missing_engine_capabilities");
    write_count(
        &mut output,
        "declarations",
        missing_capability_counts.values().copied().sum(),
    );
    for kind in EngineCapabilityKind::ALL {
        write_count(
            &mut output,
            &summary_key(kind.as_str()),
            missing_capability_counts.get(&kind).copied().unwrap_or(0),
        );
    }

    section(&mut output, "harness_limitations");
    write_count(
        &mut output,
        "declarations",
        harness_limitation_counts.values().copied().sum(),
    );
    for kind in HarnessLimitationKind::ALL {
        write_count(
            &mut output,
            &summary_key(kind.as_str()),
            harness_limitation_counts.get(&kind).copied().unwrap_or(0),
        );
    }

    section(&mut output, "environment_requirements");
    write_count(&mut output, "population", classified);
    write_count(
        &mut output,
        "tests_with_requirements",
        tests_with_environment_requirements,
    );
    write_count(
        &mut output,
        "declarations",
        environment_kind_counts.values().copied().sum(),
    );
    for kind in EnvironmentRequirementKind::ALL {
        write_count(
            &mut output,
            &summary_key(kind.as_str()),
            environment_kind_counts.get(&kind).copied().unwrap_or(0),
        );
    }
    for ((kind, profile), tests) in environment_profiles {
        output.push_str("\n[[environment_requirement_profiles]]\n");
        write_string(&mut output, "kind", kind.as_str());
        write_string(&mut output, "profile", &profile);
        write_count(&mut output, "tests", tests);
    }

    section(&mut output, "requirement_tags");
    write_count(&mut output, "population", classified);
    for tag in RequirementTag::ALL {
        write_count(
            &mut output,
            &summary_key(tag.as_str()),
            requirement_counts.get(&tag).copied().unwrap_or(0),
        );
    }

    let mut owner_counts = BTreeMap::<SubsystemOwner, usize>::new();
    for record in results.records() {
        *owner_counts.entry(record.primary_owner()).or_default() += 1;
    }
    section(&mut output, "primary_subsystem_owners");
    write_count(&mut output, "population", discovered);
    for owner in SubsystemOwner::ALL {
        write_count(
            &mut output,
            &summary_key(owner.as_str()),
            owner_counts.get(&owner).copied().unwrap_or(0),
        );
    }

    output.into_bytes()
}

fn section(output: &mut String, name: &str) {
    writeln!(output, "\n[{name}]").expect("writing to String cannot fail");
}

fn write_count(output: &mut String, key: &str, value: usize) {
    writeln!(output, "{key} = {value}").expect("writing to String cannot fail");
}

fn write_string(output: &mut String, key: &str, value: &str) {
    let encoded = toml::Value::String(value.to_owned()).to_string();
    writeln!(output, "{key} = {encoded}").expect("writing to String cannot fail");
}

fn summary_key(value: &str) -> String {
    value.replace('-', "_")
}
