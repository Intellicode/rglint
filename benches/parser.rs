//! Criterion benchmarks for the parser substrate.

use apollo_parser::cst::CstNode;
use apollo_parser::Parser;
use criterion::{black_box, criterion_group, criterion_main, Criterion};

const CORPORA: &[(&str, &str)] = &[
    (
        "github-schema",
        include_str!("corpora/github-schema.graphql"),
    ),
    (
        "shopify-schema",
        include_str!("corpora/shopify-schema.graphql"),
    ),
    (
        "recommended-query",
        include_str!("corpora/recommended-query.graphql"),
    ),
];

fn parser_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse");
    for &(name, source) in CORPORA {
        group.bench_function(name, |b| {
            b.iter(|| {
                let tree = Parser::new(black_box(source)).parse();
                black_box(tree.errors().count());
                black_box(tree.document().syntax().text_range().len());
            });
        });
    }
    group.finish();
}

criterion_group!(benches, parser_benchmarks);
criterion_main!(benches);
