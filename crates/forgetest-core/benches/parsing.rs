use criterion::{black_box, criterion_group, criterion_main, Criterion};

use forgetest_core::traits::extract_code_from_markdown;

fn bench_extract_code(c: &mut Criterion) {
    let mut group = c.benchmark_group("extract_code");

    let simple = r#"Here is the code:

```rust
fn add(a: i32, b: i32) -> i32 {
    a + b
}
```
"#;

    let multi_block = r#"First part:

```rust
fn add(a: i32, b: i32) -> i32 {
    a + b
}
```

Second part:

```rust
fn sub(a: i32, b: i32) -> i32 {
    a - b
}
```

```python
def hello():
    pass
```
"#;

    let no_blocks = "fn plain_code(x: i32) -> i32 {\n    x * 2\n}";

    let large = {
        let mut s = String::new();
        for i in 0..50 {
            s.push_str(&format!(
                "\n```rust\nfn func_{i}(x: i32) -> i32 {{ x + {i} }}\n```\n"
            ));
        }
        s
    };

    group.bench_function("simple", |b| {
        b.iter(|| extract_code_from_markdown(black_box(simple)))
    });

    group.bench_function("multi_block", |b| {
        b.iter(|| extract_code_from_markdown(black_box(multi_block)))
    });

    group.bench_function("no_blocks", |b| {
        b.iter(|| extract_code_from_markdown(black_box(no_blocks)))
    });

    group.bench_function("50_blocks", |b| {
        b.iter(|| extract_code_from_markdown(black_box(&large)))
    });

    group.finish();
}

fn bench_toml_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("toml_parsing");

    // Generate eval set TOML strings of various sizes
    let small_toml = generate_eval_set_toml(5);
    let medium_toml = generate_eval_set_toml(50);
    let large_toml = generate_eval_set_toml(200);

    group.bench_function("5_cases", |b| {
        b.iter(|| {
            forgetest_core::parser::parse_eval_set_str(
                black_box(&small_toml),
                black_box("bench.toml".as_ref()),
            )
        })
    });

    group.bench_function("50_cases", |b| {
        b.iter(|| {
            forgetest_core::parser::parse_eval_set_str(
                black_box(&medium_toml),
                black_box("bench.toml".as_ref()),
            )
        })
    });

    group.bench_function("200_cases", |b| {
        b.iter(|| {
            forgetest_core::parser::parse_eval_set_str(
                black_box(&large_toml),
                black_box("bench.toml".as_ref()),
            )
        })
    });

    group.finish();
}

fn generate_eval_set_toml(n: usize) -> String {
    let mut s = String::new();
    s.push_str(
        r#"[eval_set]
id = "bench"
name = "Benchmark"
default_language = "rust"
"#,
    );
    for i in 0..n {
        s.push_str(&format!(
            r#"
[[cases]]
id = "case_{i}"
name = "Case {i}"
prompt = "Write function {i}"
tags = ["bench"]

[cases.expectations]
should_compile = true
should_pass_tests = true
test_file = """
#[cfg(test)]
mod tests {{
    use super::*;
    #[test]
    fn test_{i}() {{ assert!(true); }}
}}
"""
expected_functions = ["func_{i}"]
"#
        ));
    }
    s
}

criterion_group!(benches, bench_extract_code, bench_toml_parsing);
criterion_main!(benches);
