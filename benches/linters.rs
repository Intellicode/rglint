//! Criterion benchmarks for full-project linting and recommended-rule costs.

use std::path::PathBuf;

use criterion::{black_box, criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion};
use rglint_config::preset::recommended;
use rglint_core::{Cache, LintEngine, Project, ProjectConfig, ProjectResolver, RulesConfig};
use rglint_rules::all_rules;

const FORCE_LINK_RGLINT_GRAPHQL_SPEC: fn() = || {
    let _ = rglint_graphql_spec::all_spec_rules();
};

fn corpus_project(schema: &str, document: &str) -> Project {
    let resolver = ProjectResolver::new(PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    resolver
        .resolve(&[ProjectConfig {
            name: schema.to_owned(),
            schema: Some(rglint_core::SchemaSpec::File(PathBuf::from(format!(
                "../../benches/corpora/{schema}-schema.graphql"
            )))),
            documents: Some(rglint_core::DocumentSpec::Files(vec![PathBuf::from(
                format!("../../benches/corpora/{document}-query.graphql"),
            )])),
            ignore: Vec::new(),
        }])
        .expect("benchmark corpus must resolve")
        .pop()
        .expect("benchmark project must exist")
}

fn recommended_rules() -> RulesConfig {
    // Both distributed-slice registries must be referenced from the benchmark
    // target or the linker can discard their entries.
    let _ = all_rules();
    let _ = FORCE_LINK_RGLINT_GRAPHQL_SPEC;
    let config = recommended();
    config.rules_config()
}

fn benchmark_engine(c: &mut Criterion, project: Project, name: &'static str) {
    let rules = recommended_rules();
    let mut group = c.benchmark_group("lint");
    group.bench_function(format!("recommended-{name}-schema"), |b| {
        b.iter_batched(
            || {
                LintEngine::new_with_cache(&rules, Cache::memory())
                    .expect("recommended preset must resolve")
            },
            |engine| black_box(engine.lint(black_box(&project)).expect("lint must succeed")),
            BatchSize::SmallInput,
        );
    });
    group.finish();
}

fn lint_benchmarks(c: &mut Criterion) {
    benchmark_engine(c, corpus_project("github", "github"), "github");
    benchmark_engine(c, corpus_project("shopify", "shopify"), "shopify");
}

fn rule_benchmarks(c: &mut Criterion) {
    let rules = recommended_rules();
    let project = corpus_project("github", "github");
    let mut configured: Vec<_> = rules.rules.iter().collect();
    configured.sort_by(|left, right| left.id.cmp(&right.id));

    let mut group = c.benchmark_group("rule");
    for rule in configured {
        let config = RulesConfig {
            rules: vec![rule.clone()],
        };
        group.bench_with_input(
            BenchmarkId::from_parameter(&rule.id),
            &config,
            |b, config| {
                b.iter_batched(
                    || {
                        LintEngine::new_with_cache(config, Cache::memory())
                            .expect("recommended rule must resolve")
                    },
                    |engine| {
                        black_box(engine.lint(black_box(&project)).expect("lint must succeed"))
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();
}

criterion_group!(benches, lint_benchmarks, rule_benchmarks);
criterion_main!(benches);
