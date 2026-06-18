use crate::models::{
    UserCamDesignRequest, UserCamDesignResult,
    DeviceInfo, CamPoint, ToleranceReport,
};
use crate::config::DynamicsConfig;
use crate::dynamics::{calculate_husking_rate, calculate_grain_breakage_rate};
use crate::optimization::{
    ToleranceAnalysis, calculate_average_pounding_force,
    evaluate_efficiency, calculate_impact_energy_per_cycle,
};

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserCamToleranceReport {
    pub min_curvature: f64,
    pub overall_feasibility: f64,
    pub manufacturing_cost: f64,
}

pub struct VrCamDesigner {
    tolerance: ToleranceAnalysis,
    dynamics_config: DynamicsConfig,
}

impl VrCamDesigner {
    pub fn new(tolerance: ToleranceAnalysis, dynamics_config: DynamicsConfig) -> Self {
        VrCamDesigner {
            tolerance,
            dynamics_config,
        }
    }

    fn build_profile_from_lifts(
        &self,
        lifts: &[f64],
        base_radius: f64,
    ) -> Vec<CamPoint> {
        let mut normalized = lifts.to_vec();
        while normalized.len() < 36 {
            let last = *normalized.last().unwrap_or(&0.0);
            normalized.push(last);
        }
        while normalized.len() > 360 {
            normalized = normalized.into_iter().step_by(2).collect();
        }

        let n = normalized.len();
        let mut points = Vec::with_capacity(n);
        for i in 0..n {
            let angle_rad = 2.0 * std::f64::consts::PI * i as f64 / n as f64;
            let angle_deg = angle_rad.to_degrees();
            let lift_val = normalized[i].max(0.0);

            let (velocity, acceleration) = if n >= 3 {
                let prev_i = (i + n - 1) % n;
                let next_i = (i + 1) % n;
                let dt = 2.0 * std::f64::consts::PI / n as f64;
                let v = (normalized[next_i] - normalized[prev_i]) / (2.0 * dt);
                let a = (normalized[next_i] - 2.0 * normalized[i] + normalized[prev_i]) / (dt * dt);
                (v, a)
            } else {
                (0.0, 0.0)
            };

            points.push(CamPoint {
                angle: angle_deg,
                radius: base_radius + lift_val,
                lift: lift_val,
                velocity,
                acceleration,
            });
        }
        points
    }

    pub fn test_user_cam_design(
        &self,
        request: &UserCamDesignRequest,
        device: &DeviceInfo,
    ) -> UserCamDesignResult {
        let profile = self.build_profile_from_lifts(
            &request.user_defined_lifts,
            request.base_radius,
        );

        let tolerance_report = self.tolerance.analyze(&profile);

        let max_lift = profile.iter().map(|p| p.lift).fold(0.0, f64::max);
        let safe_lift = max_lift.min(0.3);

        let efficiency = evaluate_efficiency(&profile, &request.grain_type, request.base_radius, safe_lift);
        let avg_force = calculate_average_pounding_force(&profile, request.base_radius, safe_lift);
        let impact_energy = calculate_impact_energy_per_cycle(safe_lift)
            * (1.0 + device.duitou_mass / 100.0);
        let husking_rate = calculate_husking_rate(impact_energy, &request.grain_type, &self.dynamics_config);
        let breakage_rate = calculate_grain_breakage_rate(impact_energy, avg_force, &self.dynamics_config);

        let mut feedback = Vec::new();
        let mut warnings = Vec::new();

        if !tolerance_report.curvature_ok {
            warnings.push("凸轮曲率半径过小，加工难度大且易磨损。".to_string());
        } else {
            feedback.push("曲率半径符合加工要求。".to_string());
        }

        if !tolerance_report.jerk_ok {
            warnings.push("加加速度(Jerk)过大，会产生剧烈冲击和噪音。".to_string());
        } else {
            feedback.push("运动平稳，冲击较小。".to_string());
        }

        if !tolerance_report.pressure_angle_ok {
            warnings.push("压力角过大，传动效率低且易卡死。".to_string());
        } else {
            feedback.push("压力角在合理范围内。".to_string());
        }

        if husking_rate < 0.6 {
            warnings.push("脱壳率偏低，可能需要增加升程或改进曲线形状。".to_string());
        } else if husking_rate > 0.9 {
            feedback.push("脱壳率优秀！".to_string());
        } else {
            feedback.push("脱壳率良好。".to_string());
        }

        if breakage_rate > 0.15 {
            warnings.push("破碎率过高，谷物损失较大。".to_string());
        } else if breakage_rate < 0.05 {
            feedback.push("破碎率控制极佳！".to_string());
        } else {
            feedback.push("破碎率在可接受范围。".to_string());
        }

        let avg_lift = profile.iter().map(|p| p.lift).sum::<f64>() / profile.len() as f64;
        if avg_lift < 0.02 {
            warnings.push("升程太小，舂捣效果不佳。".to_string());
        } else if avg_lift > 0.25 {
            warnings.push("升程过大，能耗高且可能损坏设备。".to_string());
        }

        let score = if tolerance_report.overall_feasibility > 0.0 {
            efficiency * 0.6 + tolerance_report.overall_feasibility * 0.4
        } else {
            efficiency * 0.6
        };

        let grade = if score >= 0.85 {
            "S级 - 大师级设计".to_string()
        } else if score >= 0.7 {
            "A级 - 优秀设计".to_string()
        } else if score >= 0.55 {
            "B级 - 良好设计".to_string()
        } else if score >= 0.4 {
            "C级 - 合格设计".to_string()
        } else {
            "D级 - 需要改进".to_string()
        };

        UserCamDesignResult {
            design_id: Uuid::new_v4().to_string(),
            design_name: request.design_name.clone().unwrap_or_else(|| "用户设计".to_string()),
            overall_efficiency: efficiency,
            husking_rate,
            breakage_rate,
            pounding_force: avg_force,
            impact_energy,
            tolerance_report,
            cam_profile: profile,
            design_feedback: feedback,
            safety_warnings: warnings,
            overall_score: score,
            grade,
            timestamp: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::optimization::create_test_device;

    fn make_simple_harmonic_lifts(n: usize, max_lift: f64) -> Vec<f64> {
        (0..n).map(|i| {
            let t = i as f64 / n as f64;
            max_lift * 0.5 * (1.0 - (2.0 * std::f64::consts::PI * t).cos())
        }).collect()
    }

    fn make_request(lifts: Vec<f64>) -> UserCamDesignRequest {
        UserCamDesignRequest {
            user_id: None,
            design_name: Some("测试设计".to_string()),
            base_radius: 0.15,
            grain_type: "rice".to_string(),
            user_defined_lifts: lifts,
        }
    }

    #[test]
    fn test_vr_designer_basic() {
        let tolerance = ToleranceAnalysis::default();
        let dynamics = DynamicsConfig::default();
        let designer = VrCamDesigner::new(tolerance, dynamics);
        let device = create_test_device();

        let lifts = make_simple_harmonic_lifts(72, 0.12);
        let req = make_request(lifts);
        let result = designer.test_user_cam_design(&req, &device);

        assert_eq!(result.design_name, "测试设计");
        assert!(result.overall_efficiency > 0.0 && result.overall_efficiency <= 1.0);
        assert!(result.husking_rate > 0.0 && result.husking_rate <= 1.0);
        assert!(result.breakage_rate >= 0.0 && result.breakage_rate < 1.0);
        assert!(result.overall_score > 0.0 && result.overall_score <= 1.0);
        assert!(result.grade.contains('级'));
        assert!(!result.design_feedback.is_empty());
    }

    #[test]
    fn test_vr_designer_grade_boundaries() {
        let tolerance = ToleranceAnalysis::default();
        let dynamics = DynamicsConfig::default();
        let designer = VrCamDesigner::new(tolerance, dynamics);
        let device = create_test_device();

        let good_lifts = make_simple_harmonic_lifts(72, 0.12);
        let result = designer.test_user_cam_design(&make_request(good_lifts), &device);
        let valid_grades = ["S级", "A级", "B级", "C级", "D级"];
        let starts_with_valid = valid_grades.iter().any(|g| result.grade.starts_with(g.chars().next().unwrap()));
        assert!(starts_with_valid,
            "设计等级应为S/A/B/C/D其中之一，实际为{}", result.grade);
        assert!(result.overall_score > 0.0 && result.overall_score <= 1.0);
    }

    #[test]
    fn test_vr_designer_zero_lifts() {
        let tolerance = ToleranceAnalysis::default();
        let dynamics = DynamicsConfig::default();
        let designer = VrCamDesigner::new(tolerance, dynamics);
        let device = create_test_device();

        let lifts = vec![0.0; 72];
        let result = designer.test_user_cam_design(&make_request(lifts), &device);
        assert!(result.impact_energy >= 0.0);
        assert!(!result.safety_warnings.is_empty());
    }

    #[test]
    fn test_vr_designer_tolerance_report() {
        let tolerance = ToleranceAnalysis::default();
        let dynamics = DynamicsConfig::default();
        let designer = VrCamDesigner::new(tolerance, dynamics);
        let device = create_test_device();

        let lifts = make_simple_harmonic_lifts(72, 0.10);
        let result = designer.test_user_cam_design(&make_request(lifts), &device);
        assert!(result.tolerance_report.min_curvature > 0.0);
        assert!(result.tolerance_report.overall_feasibility >= 0.0);
        assert!(result.tolerance_report.manufacturing_cost > 0.0);
    }

    #[test]
    fn test_vr_designer_profile_normalization() {
        let tolerance = ToleranceAnalysis::default();
        let dynamics = DynamicsConfig::default();
        let designer = VrCamDesigner::new(tolerance, dynamics);

        let short_lifts = vec![0.1; 10];
        let profile = designer.build_profile_from_lifts(&short_lifts, 0.15);
        assert!(profile.len() >= 36, "短输入应补齐到至少36点");

        let long_lifts: Vec<f64> = (0..500).map(|i| 0.1 * (i as f64 / 500.0)).collect();
        let profile = designer.build_profile_from_lifts(&long_lifts, 0.15);
        assert!(profile.len() <= 360, "长输入应降采样到360点以内");
    }
}
