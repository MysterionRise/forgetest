use criterion::{black_box, criterion_group, criterion_main, Criterion};

use forgetest_core::model::Expectations;
use forgetest_core::results::*;
use forgetest_core::statistics::pass_at_k;
use uuid::Uuid;

fn make_result(compile_ok: bool, passed: u32, failed: u32, warnings: u32) -> EvalResult {
    EvalResult {
        case_id: "bench".into(),
        model: "bench-model".into(),
        provider: "bench".into(),
        generated_code: String::new(),
        compilation: CompilationResult {
            success: compile_ok,
            errors: vec![],
            warnings: vec![],
            duration_ms: 0,
        },
        test_execution: if compile_ok {
            Some(TestResult {
                passed,
                failed,
                ignored: 0,
                duration_ms: 0,
                failures: vec![],
            })
        } else {
            None
        },
        clippy: Some(ClippyResult {
            warnings: vec![],
            warning_count: warnings,
        }),
        timing: TimingInfo {
            llm_request_ms: 0,
            compilation_ms: 0,
            test_execution_ms: 0,
            total_ms: 0,
        },
        token_usage: TokenUsage {
            prompt_tokens: 100,
            completion_tokens: 200,
            total_tokens: 300,
            estimated_cost_usd: 0.01,
        },
        attempt: 1,
        run_id: Uuid::nil(),
    }
}

fn bench_pass_at_k(c: &mut Criterion) {
    let mut group = c.benchmark_group("pass_at_k");

    group.bench_function("n=10,c=5,k=1", |b| {
        b.iter(|| pass_at_k(black_box(10), black_box(5), black_box(1)))
    });

    group.bench_function("n=100,c=50,k=10", |b| {
        b.iter(|| pass_at_k(black_box(100), black_box(50), black_box(10)))
    });

    group.bench_function("n=1000,c=500,k=100", |b| {
        b.iter(|| pass_at_k(black_box(1000), black_box(500), black_box(100)))
    });

    group.finish();
}

fn bench_score_compute(c: &mut Criterion) {
    let mut group = c.benchmark_group("score_compute");
    let expectations = Expectations::default();

    group.bench_function("perfect", |b| {
        let result = make_result(true, 5, 0, 0);
        b.iter(|| Score::compute(black_box(&result), black_box(&expectations)))
    });

    group.bench_function("compile_fail", |b| {
        let result = make_result(false, 0, 0, 0);
        b.iter(|| Score::compute(black_box(&result), black_box(&expectations)))
    });

    group.bench_function("partial_tests", |b| {
        let result = make_result(true, 3, 2, 1);
        b.iter(|| Score::compute(black_box(&result), black_box(&expectations)))
    });

    group.finish();
}

criterion_group!(benches, bench_pass_at_k, bench_score_compute);
criterion_main!(benches);
