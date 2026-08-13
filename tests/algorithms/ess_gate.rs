use kesa::extension_scoring::{
    OpeEvaluatorConfig, OpeGateReason, OpeTraceSample, evaluate_off_policy,
};

fn sample_with_weight(action: &str, behavior_propensity: f64) -> OpeTraceSample {
    OpeTraceSample {
        action: action.to_string(),
        behavior_propensity,
        target_propensity: 1.0,
        outcome: 1.0,
        baseline_outcome: Some(1.0),
        direct_method_prediction: Some(1.0),
        context_lineage: Some(format!("ctx:{action}")),
    }
}

const fn ess_gate_config(min_effective_sample_size: f64) -> OpeEvaluatorConfig {
    OpeEvaluatorConfig {
        max_importance_weight: 100.0,
        min_effective_sample_size,
        max_standard_error: 10.0,
        confidence_z: 1.96,
        max_regret_delta: 10.0,
    }
}

#[test]
fn equal_importance_weights_preserve_effective_sample_size_and_pass_gate() {
    let samples = (0..16)
        .map(|idx| sample_with_weight(&format!("balanced-{idx}"), 1.0))
        .collect::<Vec<_>>();
    let report = evaluate_off_policy(&samples, &ess_gate_config(8.0));

    assert_eq!(report.gate.reason, OpeGateReason::Approved);
    assert!(report.gate.passed);
    assert_eq!(report.diagnostics.valid_samples, samples.len());
    assert!(
        (report.diagnostics.effective_sample_size - 16.0).abs() <= 1e-12,
        "equal weights should keep ESS at n; got {}",
        report.diagnostics.effective_sample_size
    );
}

#[test]
fn dominant_importance_weight_collapses_effective_sample_size_and_denies_gate() {
    let mut samples = vec![sample_with_weight("dominant", 0.01)];
    samples.extend((0..15).map(|idx| sample_with_weight(&format!("ordinary-{idx}"), 1.0)));

    let report = evaluate_off_policy(&samples, &ess_gate_config(8.0));

    assert_eq!(report.gate.reason, OpeGateReason::InsufficientSupport);
    assert!(!report.gate.passed);
    assert_eq!(report.diagnostics.valid_samples, samples.len());
    assert!(
        report.diagnostics.effective_sample_size < 2.0,
        "one 100x weight should collapse ESS below the support threshold; got {}",
        report.diagnostics.effective_sample_size
    );
}
