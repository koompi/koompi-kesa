//! Focused Criterion inputs for extension performance budgets.
//!
//! This bench emits only the extension artifacts consumed by
//! `tests/perf_budgets.rs`, so perf evidence refreshes do not have to run the
//! full extension benchmark graph.

#[path = "bench_env.rs"]
mod bench_env;

use std::hint::black_box;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use futures::executor::block_on;
use kode::extensions::{ExtensionManager, JsExtensionLoadSpec, JsExtensionRuntimeHandle};
use kode::extensions_js::PiJsRuntimeConfig;
use kode::tools::ToolRegistry;

fn criterion_config() -> Criterion {
    bench_env::criterion_config()
}

fn artifact_single_file_entry(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/ext_conformance/artifacts")
        .join(name)
        .join(format!("{name}.ts"))
}

fn bench_extension_policy(c: &mut Criterion) {
    let prompt = kode::extensions::ExtensionPolicy::default();
    let strict = kode::extensions::ExtensionPolicy {
        mode: kode::extensions::ExtensionPolicyMode::Strict,
        ..kode::extensions::ExtensionPolicy::default()
    };
    let permissive = kode::extensions::ExtensionPolicy {
        mode: kode::extensions::ExtensionPolicyMode::Permissive,
        ..kode::extensions::ExtensionPolicy::default()
    };

    let cases: Vec<(&str, &kode::extensions::ExtensionPolicy, &str)> = vec![
        ("prompt_allow", &prompt, "read"),
        ("prompt_prompt", &prompt, "session"),
        ("prompt_deny", &prompt, "exec"),
        ("strict_allow", &strict, "read"),
        ("strict_deny", &strict, "session"),
        ("permissive_allow", &permissive, "env"),
    ];

    let mut group = c.benchmark_group("ext_policy");
    for (name, policy, cap) in cases {
        group.bench_function(BenchmarkId::new("evaluate", name), |b| {
            b.iter(|| black_box(policy.evaluate(black_box(cap))));
        });
    }
    group.finish();
}

fn bench_protocol_parse_and_validate(c: &mut Criterion) {
    let host_call_small = format!(
        r#"{{"id":"msg-1","version":"{}","type":"host_call","payload":{{"call_id":"call-1","capability":"read","method":"tool","params":{{"name":"read"}}}}}}"#,
        kode::extensions::PROTOCOL_VERSION
    );

    let big_text = "x".repeat(16 * 1024);
    let log_big = format!(
        r#"{{"id":"msg-2","version":"{}","type":"log","payload":{{"schema":"{}","ts":"2026-02-03T00:00:00.000Z","level":"info","event":"bench","message":"{}","correlation":{{"extension_id":"ext","scenario_id":"scn"}},"source":{{"component":"runtime"}}}}}}"#,
        kode::extensions::PROTOCOL_VERSION,
        kode::extensions::LOG_SCHEMA_VERSION,
        big_text
    );

    let cases: Vec<(&str, &str)> =
        vec![("host_call_small", &host_call_small), ("log_big", &log_big)];

    let mut group = c.benchmark_group("ext_protocol");
    for (name, payload) in cases {
        group.throughput(Throughput::Bytes(payload.len() as u64));
        group.bench_function(BenchmarkId::new("parse_and_validate", name), |b| {
            b.iter(|| {
                black_box(kode::extensions::ExtensionMessage::parse_and_validate(
                    payload,
                ))
            });
        });
    }
    group.finish();
}

fn bench_extension_load_init(c: &mut Criterion) {
    let cwd = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let js_cwd = cwd.display().to_string();

    let mut group = c.benchmark_group("ext_load_init");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(10));

    let entry_path = artifact_single_file_entry("hello");
    let spec = JsExtensionLoadSpec::from_entry_path(&entry_path)
        .unwrap_or_else(|_| panic!("expected hello artifact at {}", entry_path.display()));

    group.bench_function(BenchmarkId::new("load_init_cold", "hello"), move |b| {
        let spec = spec.clone();
        let cwd = cwd.clone();
        let js_cwd = js_cwd.clone();

        b.iter_batched(
            || {
                let manager = ExtensionManager::new();
                let tools = Arc::new(ToolRegistry::new(&[], &cwd, None));
                let runtime = block_on({
                    let manager = manager.clone();
                    let tools = Arc::clone(&tools);
                    let js_config = PiJsRuntimeConfig {
                        cwd: js_cwd.clone(),
                        ..Default::default()
                    };
                    async move {
                        JsExtensionRuntimeHandle::start(js_config, tools, manager)
                            .await
                            .expect("start js runtime")
                    }
                });
                manager.set_js_runtime(runtime);
                manager
            },
            |manager| {
                block_on({
                    let spec = spec.clone();
                    async move {
                        manager
                            .load_js_extensions(vec![spec])
                            .await
                            .expect("load hello extension");
                        let _ok = manager.shutdown(Duration::from_millis(250)).await;
                    }
                });
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

criterion_group!(
    name = benches;
    config = criterion_config();
    targets =
        bench_extension_load_init,
        bench_extension_policy,
        bench_protocol_parse_and_validate
);
criterion_main!(benches);
