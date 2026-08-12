use kode::extension_dispatcher::{DualExecOracleConfig, DualExecOracleState};

const fn backoff_test_config() -> DualExecOracleConfig {
    DualExecOracleConfig {
        sample_ppm: 1_000_000,
        divergence_window: 4,
        divergence_budget: 2,
        rollback_requests: 5,
        overhead_budget_us: u64::MAX,
        overhead_backoff_requests: 3,
    }
}

#[test]
fn forced_divergence_triggers_policy_backoff_for_configured_request_count() {
    let config = backoff_test_config();
    let mut state = DualExecOracleState::default();

    assert!(
        state
            .record_sample(false, config, Some("ext.clean"))
            .is_none()
    );
    assert!(
        state
            .record_sample(true, config, Some("ext.forced"))
            .is_none()
    );

    let reason = state
        .record_sample(true, config, Some("ext.forced"))
        .expect("second divergence within the window must trigger rollback backoff");

    assert_eq!(state.sampled_total(), 3);
    assert_eq!(state.matched_total(), 1);
    assert_eq!(state.divergence_total(), 2);
    assert!(state.rollback_active());
    assert_eq!(state.rollback_remaining(), config.rollback_requests);
    assert_eq!(state.rollback_reason(), Some(reason.as_str()));
    assert!(
        reason.contains("2/3:ext.forced"),
        "reason must preserve divergence count, window size, and extension scope: {reason}"
    );

    for expected_remaining in (0..config.rollback_requests).rev() {
        state.begin_request();
        assert_eq!(state.rollback_remaining(), expected_remaining);
    }

    assert!(!state.rollback_active());
    assert_eq!(state.rollback_reason(), None);
}

#[test]
fn matching_shadow_samples_do_not_trigger_divergence_backoff() {
    let config = backoff_test_config();
    let mut state = DualExecOracleState::default();

    for idx in 0..(config.divergence_window * 2) {
        assert!(
            state
                .record_sample(false, config, Some("ext.clean"))
                .is_none(),
            "non-divergent sample {idx} must not trigger rollback backoff"
        );
    }

    assert_eq!(state.sampled_total(), 8);
    assert_eq!(state.matched_total(), 8);
    assert_eq!(state.divergence_total(), 0);
    assert!(!state.rollback_active());
    assert_eq!(state.rollback_remaining(), 0);
    assert_eq!(state.rollback_reason(), None);
    assert_eq!(state.overhead_backoff_remaining(), 0);
    assert_eq!(state.skipped_overhead_total(), 0);
}
