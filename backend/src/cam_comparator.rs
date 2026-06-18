use crate::models::{
    CamProfileComparisonRequest, CamProfileComparisonResult,
    ProfileEfficiencyResult, DeviceInfo, CamPoint,
};
use crate::config::DynamicsConfig;
use crate::dynamics::{calculate_husking_rate, calculate_grain_breakage_rate};
use crate::optimization::{
    ToleranceAnalysis, calculate_average_pounding_force,
    generate_profile, evaluate_efficiency, calculate_impact_energy_per_cycle,
};

use uuid::Uuid;
use chrono::{DateTime, Utc};

const PROFILE_NAMES_CN: &[(&str, &str)] = &[
    ("cycloidal", "摆线凸轮"),
    ("harmonic", "简谐凸轮"),
    ("polynomial", "3-4-5次多项式凸轮"),
    ("involute", "渐开线凸轮"),
    ("circular_arc", "圆弧凸轮"),
];

pub struct CamComparator {
    tolerance: ToleranceAnalysis,
    dynamics_config: DynamicsConfig,
}

impl CamComparator {
    pub fn new(tolerance: ToleranceAnalysis, dynamics_config: DynamicsConfig) -> Self {
        CamComparator {
            tolerance,
            dynamics_config,
        }
    }

    pub fn compare_profiles(
        &self,
        request: &CamProfileComparisonRequest,
        device: &DeviceInfo,
    ) -> CamProfileComparisonResult {
        let mut results = Vec::new();

        if request.profile_types.is_empty() {
            return CamProfileComparisonResult {
                comparison_id: Uuid::new_v4().to_string(),
                device_id: request.device_id.clone(),
                grain_type: request.grain_type.clone(),
                results,
                best_profile: String::new(),
                timestamp: Utc::now(),
            };
        }

        let profile_types = request.profile_types.clone();

        for profile_type in &profile_types {
            let profile_name_cn = PROFILE_NAMES_CN
                .iter()
                .find(|(k, _)| k == profile_type)
                .map(|(_, v)| *v)
                .unwrap_or("未知类型");

            let profile = generate_profile(
                profile_type,
                request.base_radius,
                request.lift,
            );

            let tolerance_report = self.tolerance.analyze(&profile);

            let overall_efficiency = evaluate_efficiency(
                &profile,
                &request.grain_type,
                request.base_radius,
                request.lift,
            );

            let cost_factor = (1000.0 / tolerance_report.manufacturing_cost)
                .max(0.3).min(1.0);
            let tolerance_factor = tolerance_report.overall_feasibility.max(0.3);
            let score = overall_efficiency * cost_factor * tolerance_factor * 0.7 + overall_efficiency * 0.3;

            let avg_force = calculate_average_pounding_force(&profile, request.base_radius, request.lift);
            let impact_energy = calculate_impact_energy_per_cycle(request.lift);
            let husking_rate = calculate_husking_rate(impact_energy, &request.grain_type, &self.dynamics_config);
            let breakage_rate = calculate_grain_breakage_rate(impact_energy, avg_force, &self.dynamics_config);

            let max_jerk = profile.windows(3)
                .map(|w| {
                    let dt = 2.0 * std::f64::consts::PI / 360.0;
                    let da1 = (w[1].acceleration - w[0].acceleration) / dt;
                    let da2 = (w[2].acceleration - w[1].acceleration) / dt;
                    ((da2 - da1) / dt).abs()
                })
                .fold(0.0, f64::max);

            let max_pressure_angle = (std::f64::consts::PI * request.lift / 2.0 / request.base_radius).atan();

            results.push(ProfileEfficiencyResult {
                profile_type: profile_type.clone(),
                profile_name_cn: profile_name_cn.to_string(),
                overall_efficiency,
                husking_rate,
                breakage_rate,
                pounding_force: avg_force,
                impact_energy,
                max_jerk,
                max_pressure_angle: max_pressure_angle.to_degrees(),
                min_curvature: tolerance_report.min_curvature,
                manufacturing_cost: tolerance_report.manufacturing_cost,
                cam_profile: profile,
                score,
            });
        }

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        let best_profile = results.first().map(|r| r.profile_type.clone()).unwrap_or_default();

        CamProfileComparisonResult {
            comparison_id: Uuid::new_v4().to_string(),
            device_id: request.device_id.clone(),
            grain_type: request.grain_type.clone(),
            results,
            best_profile,
            timestamp: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::optimization::create_test_device;

    fn make_test_request() -> CamProfileComparisonRequest {
        CamProfileComparisonRequest {
            device_id: "test-001".to_string(),
            grain_type: "rice".to_string(),
            profile_types: vec![
                "cycloidal".to_string(),
                "harmonic".to_string(),
                "involute".to_string(),
                "circular_arc".to_string(),
            ],
            base_radius: 0.15,
            lift: 0.12,
        }
    }

    #[test]
    fn test_cam_comparator_basic() {
        let device = create_test_device();
        let tolerance = ToleranceAnalysis::default();
        let dynamics = DynamicsConfig::default();
        let comparator = CamComparator::new(tolerance, dynamics);

        let result = comparator.compare_profiles(&make_test_request(), &device);
        assert_eq!(result.results.len(), 4);
        assert!(!result.best_profile.is_empty());
    }

    #[test]
    fn test_cam_comparator_empty_profiles() {
        let device = create_test_device();
        let tolerance = ToleranceAnalysis::default();
        let dynamics = DynamicsConfig::default();
        let comparator = CamComparator::new(tolerance, dynamics);

        let mut req = make_test_request();
        req.profile_types = vec![];
        let result = comparator.compare_profiles(&req, &device);
        assert_eq!(result.results.len(), 0);
        assert_eq!(result.best_profile, "");
    }

    #[test]
    fn test_cam_comparator_ranking() {
        let device = create_test_device();
        let tolerance = ToleranceAnalysis::default();
        let dynamics = DynamicsConfig::default();
        let comparator = CamComparator::new(tolerance, dynamics);

        let result = comparator.compare_profiles(&make_test_request(), &device);
        for i in 0..result.results.len() - 1 {
            assert!(result.results[i].score >= result.results[i + 1].score);
        }
    }

    #[test]
    fn test_cam_comparator_scores_in_range() {
        let device = create_test_device();
        let tolerance = ToleranceAnalysis::default();
        let dynamics = DynamicsConfig::default();
        let comparator = CamComparator::new(tolerance, dynamics);

        let result = comparator.compare_profiles(&make_test_request(), &device);
        for r in &result.results {
            assert!(r.overall_efficiency > 0.0 && r.overall_efficiency <= 1.0);
            assert!(r.husking_rate > 0.0 && r.husking_rate <= 1.0);
            assert!(r.breakage_rate >= 0.0 && r.breakage_rate < 1.0);
            assert!(r.impact_energy > 0.0);
        }
    }
}
