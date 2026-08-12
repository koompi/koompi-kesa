use chrono::{TimeZone, Utc};
use kode::extension_scoring::{VoiCandidate, VoiPlannerConfig, plan_voi_candidates};

const CANDIDATES_PER_SEED: usize = 24;
const SEED_COUNT: u64 = 128;
const MIN_APPROXIMATION_RATIO: f64 = 0.8;

#[test]
fn voi_planner_matches_budgeted_utility_frontier_over_seeded_cases() {
    let now = Utc
        .with_ymd_and_hms(2026, 4, 29, 12, 0, 0)
        .single()
        .expect("valid timestamp");
    let fresh = Utc
        .with_ymd_and_hms(2026, 4, 29, 11, 45, 0)
        .single()
        .expect("valid timestamp")
        .to_rfc3339();
    let config = VoiPlannerConfig {
        enabled: true,
        overhead_budget_ms: 64,
        max_candidates: None,
        stale_after_minutes: Some(120),
        min_utility_score: Some(0.0),
    };
    let mut min_ratio = f64::INFINITY;

    for seed in 0..SEED_COUNT {
        let candidates = seeded_candidates(seed, &fresh);
        assert_eq!(candidates.len(), CANDIDATES_PER_SEED);

        let plan = plan_voi_candidates(&candidates, now, &config);
        assert!(plan.used_overhead_ms <= config.overhead_budget_ms);
        assert_selected_by_priority_order(&plan.selected);

        let selected_utility = plan
            .selected
            .iter()
            .map(|candidate| candidate.utility_score)
            .sum::<f64>();
        let optimal_utility = optimal_budgeted_utility(&candidates, config.overhead_budget_ms);
        let ratio = if optimal_utility <= f64::EPSILON {
            1.0
        } else {
            selected_utility / optimal_utility
        };
        min_ratio = min_ratio.min(ratio);

        assert!(
            ratio >= MIN_APPROXIMATION_RATIO,
            "seed {seed} ratio {ratio:.3} below {MIN_APPROXIMATION_RATIO}; selected={selected_utility:.3}, optimal={optimal_utility:.3}"
        );
    }

    assert!(
        min_ratio >= MIN_APPROXIMATION_RATIO,
        "minimum observed ratio {min_ratio:.3} below {MIN_APPROXIMATION_RATIO}"
    );
}

fn seeded_candidates(seed: u64, fresh: &str) -> Vec<VoiCandidate> {
    let mut rng = DeterministicRng::new(seed);
    (0..CANDIDATES_PER_SEED)
        .map(|index| VoiCandidate {
            id: format!("seed-{seed}-candidate-{index:02}"),
            utility_score: f64::from(rng.range_inclusive(1, 240)) / 10.0,
            estimated_overhead_ms: rng.range_inclusive(1, 24),
            last_seen_at: Some(fresh.to_string()),
            enabled: true,
        })
        .collect()
}

fn assert_selected_by_priority_order(selected: &[kode::extension_scoring::VoiPlannedCandidate]) {
    for window in selected.windows(2) {
        assert!(
            window[0].utility_per_ms >= window[1].utility_per_ms,
            "selected candidates should remain explainable in utility/ms order: {} ({}) before {} ({})",
            window[0].id,
            window[0].utility_per_ms,
            window[1].id,
            window[1].utility_per_ms
        );
    }
}

fn optimal_budgeted_utility(candidates: &[VoiCandidate], overhead_budget_ms: u32) -> f64 {
    let budget = usize::try_from(overhead_budget_ms).expect("budget fits usize");
    let mut best_by_overhead = vec![0.0; budget + 1];

    for candidate in candidates {
        let overhead = usize::try_from(candidate.estimated_overhead_ms).expect("overhead fits");
        if overhead > budget {
            continue;
        }
        let utility = candidate.utility_score.max(0.0);
        for used in (overhead..=budget).rev() {
            let with_candidate = best_by_overhead[used - overhead] + utility;
            if with_candidate > best_by_overhead[used] {
                best_by_overhead[used] = with_candidate;
            }
        }
    }

    best_by_overhead.into_iter().fold(0.0_f64, f64::max)
}

struct DeterministicRng {
    state: u64,
}

impl DeterministicRng {
    const fn new(seed: u64) -> Self {
        Self {
            state: seed ^ 0xa076_1d64_78bd_642f,
        }
    }

    const fn next(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(0xe703_7ed1_a0b4_28db)
            .wrapping_add(0x8ebc_6af0_9c88_c6e3);
        self.state ^ (self.state >> 32)
    }

    fn range_inclusive(&mut self, min: u32, max: u32) -> u32 {
        debug_assert!(min <= max);
        let span = u64::from(max - min + 1);
        min + u32::try_from(self.next() % span).expect("range fits u32")
    }
}
