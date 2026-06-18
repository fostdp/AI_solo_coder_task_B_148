use crate::models::{
    OptimizationRequest, OptimizationResult, CamPoint, DeviceInfo, ToleranceReport,
    CamProfileComparisonRequest, CamProfileComparisonResult,
    CrossEraComparisonRequest, CrossEraComparisonResult,
    VibrationInterferenceRequest, VibrationInterferenceResult,
    UserCamDesignRequest, UserCamDesignResult,
};
use crate::dynamics::{calculate_husking_rate, calculate_grain_breakage_rate};
use crate::config::{OptimizationConfig, ToleranceConfig, DynamicsConfig};
use crate::cam_comparator::CamComparator;
use crate::era_comparator::EraComparator;
use crate::vr_cam_designer::VrCamDesigner;
use chrono::Utc;
use uuid::Uuid;

pub use crate::era_comparator::{
    get_ancient_archaeological_specs, get_modern_standard_specs,
    AncientShuiduiSpecs, ModernRiceMillStandard,
};
pub use crate::vibration_interference::{
    get_foundation_props_for_type, foundation_transmissibility,
    VibrationInterferenceAnalyzer,
};
pub use crate::cam_comparator::CamComparator as CamComparisonEngine;
pub use crate::vr_cam_designer::VrCamDesigner as VirtualCamDesigner;
pub use crate::vr_cam_designer::UserCamToleranceReport;
pub use crate::dynamics_pool::{DynamicsPool, DynamicsTask, DynamicsTaskResult};

const GRAVITY: f64 = 9.81;
const MANUFACTURING_COST_BASE: f64 = 1000.0;

#[derive(Clone)]
pub struct ToleranceAnalysis {
    dimensional_tolerance: f64,
    surface_roughness: f64,
    angular_tolerance: f64,
    min_curvature_radius: f64,
    jerk_limit: f64,
    manufacturing_cost_base: f64,
}

impl Default for ToleranceAnalysis {
    fn default() -> Self {
        let cfg = OptimizationConfig::load_default();
        Self::from_config(&cfg.tolerance)
    }
}

impl ToleranceAnalysis {
    pub fn from_config(cfg: &ToleranceConfig) -> Self {
        ToleranceAnalysis {
            dimensional_tolerance: cfg.dimensional_tolerance_meters,
            surface_roughness: cfg.surface_roughness_ra_meters,
            angular_tolerance: cfg.angular_tolerance_radians,
            min_curvature_radius: cfg.min_curvature_radius_meters,
            jerk_limit: cfg.jerk_limit_m_per_s3,
            manufacturing_cost_base: cfg.manufacturing_cost_base_cny,
        }
    }

    pub fn new(
        dimensional_tolerance: f64,
        surface_roughness: f64,
        angular_tolerance: f64,
        min_curvature_radius: f64,
    ) -> Self {
        ToleranceAnalysis {
            dimensional_tolerance,
            surface_roughness,
            angular_tolerance,
            min_curvature_radius,
            jerk_limit: 1000.0,
            manufacturing_cost_base: 1000.0,
        }
    }

    pub fn analyze(&self, profile: &[CamPoint]) -> ToleranceReport {
        let mut report = ToleranceReport {
            min_curvature: 0.0,
            lift_deviation: 0.0,
            pressure_angle_variation: 0.0,
            surface_sensitivity: 0.0,
            jerk: 0.0,
            overall_feasibility: 0.0,
            manufacturing_cost: 0.0,
            curvature_ok: false,
            lift_ok: false,
            pressure_angle_ok: false,
            jerk_ok: false,
            surface_ok: false,
        };

        report.min_curvature = self.calculate_min_curvature(profile);
        report.curvature_ok = report.min_curvature >= self.min_curvature_radius;

        report.lift_deviation = self.calculate_lift_tolerance_sensitivity(profile);
        report.lift_ok = report.lift_deviation <= self.dimensional_tolerance * 5.0;

        report.pressure_angle_variation = self.calculate_pressure_angle_variation(profile);
        report.pressure_angle_ok = report.pressure_angle_variation <= self.angular_tolerance;

        report.surface_sensitivity = self.calculate_surface_sensitivity(profile);
        report.surface_ok = report.surface_sensitivity <= self.surface_roughness * 100.0;

        report.jerk = self.calculate_max_jerk(profile);
        report.jerk_ok = report.jerk < self.jerk_limit;

        report.overall_feasibility =
            report.curvature_ok as u8 as f64 * 0.35 +
            report.lift_ok as u8 as f64 * 0.25 +
            report.pressure_angle_ok as u8 as f64 * 0.2 +
            report.jerk_ok as u8 as f64 * 0.1 +
            report.surface_ok as u8 as f64 * 0.1;

        report.manufacturing_cost = self.estimate_manufacturing_cost(&report, profile);

        report
    }

    fn calculate_min_curvature(&self, profile: &[CamPoint]) -> f64 {
        let n = profile.len();
        let mut min_r = f64::INFINITY;

        for i in 0..n {
            let i0 = (i + n - 1) % n;
            let i1 = i;
            let i2 = (i + 1) % n;

            let p0 = &profile[i0];
            let p1 = &profile[i1];
            let p2 = &profile[i2];

            let a0 = p0.angle.to_radians();
            let a1 = p1.angle.to_radians();
            let a2 = p2.angle.to_radians();

            let x0 = p0.radius * a0.cos();
            let y0 = p0.radius * a0.sin();
            let x1 = p1.radius * a1.cos();
            let y1 = p1.radius * a1.sin();
            let x2 = p2.radius * a2.cos();
            let y2 = p2.radius * a2.sin();

            let curvature = Self::curvature_from_points(x0, y0, x1, y1, x2, y2);
            if curvature > 0.0 {
                let r = 1.0 / curvature;
                if r < min_r {
                    min_r = r;
                }
            }
        }

        min_r
    }

    fn curvature_from_points(
        x0: f64, y0: f64,
        x1: f64, y1: f64,
        x2: f64, y2: f64,
    ) -> f64 {
        let dx1 = x1 - x0;
        let dy1 = y1 - y0;
        let dx2 = x2 - x1;
        let dy2 = y2 - y1;

        let cross = dx1 * dy2 - dy1 * dx2;
        let d1 = (dx1 * dx1 + dy1 * dy1).sqrt();
        let d2 = (dx2 * dx2 + dy2 * dy2).sqrt();
        let d12 = ((x2 - x0) * (x2 - x0) + (y2 - y0) * (y2 - y0)).sqrt();

        if d1 < 1e-10 || d2 < 1e-10 || d12 < 1e-10 {
            return 0.0;
        }

        2.0 * cross.abs() / (d1 * d2 * d12)
    }

    fn calculate_lift_tolerance_sensitivity(&self, profile: &[CamPoint]) -> f64 {
        if profile.len() < 3 {
            return 0.0;
        }

        let lifts: Vec<f64> = profile.iter().map(|p| p.lift).collect();
        let mean_lift: f64 = lifts.iter().sum::<f64>() / lifts.len() as f64;

        let variance: f64 = lifts
            .iter()
            .map(|l| (l - mean_lift).powi(2))
            .sum::<f64>() / lifts.len() as f64;

        variance.sqrt()
    }

    fn calculate_pressure_angle_variation(&self, profile: &[CamPoint]) -> f64 {
        if profile.len() < 2 {
            return 0.0;
        }

        let mut max_variation = 0.0;

        for i in 1..profile.len() {
            let _prev = &profile[i - 1];
            let curr = &profile[i];

            if curr.radius > 0.0 {
                let variation = (curr.velocity / curr.radius).abs();
                if variation > max_variation {
                    max_variation = variation;
                }
            }
        }

        max_variation
    }

    fn calculate_surface_sensitivity(&self, profile: &[CamPoint]) -> f64 {
        if profile.len() < 2 {
            return 0.0;
        }

        let mut total_variation = 0.0;

        for i in 1..profile.len() {
            let prev = &profile[i - 1];
            let curr = &profile[i];
            let d_angle = (curr.angle - prev.angle).to_radians();

            let dr = (curr.radius - prev.radius).abs();
            let arc_length = ((curr.radius + prev.radius) / 2.0) * d_angle.abs();

            if arc_length > 0.0 {
                total_variation += dr / arc_length;
            }
        }

        total_variation / profile.len() as f64
    }

    fn calculate_max_jerk(&self, profile: &[CamPoint]) -> f64 {
        if profile.len() < 3 {
            return 0.0;
        }

        let mut max_jerk = 0.0;

        for i in 2..profile.len() {
            let a0 = profile[i - 2].acceleration;
            let a1 = profile[i - 1].acceleration;
            let a2 = profile[i].acceleration;

            let t0 = profile[i - 2].angle.to_radians();
            let t1 = profile[i - 1].angle.to_radians();
            let t2 = profile[i].angle.to_radians();

            let dt1 = t1 - t0;
            let dt2 = t2 - t1;

            if dt1 > 0.0 && dt2 > 0.0 {
                let da1 = (a1 - a0) / dt1;
                let da2 = (a2 - a1) / dt2;
                let jerk = ((da2 - da1) / ((dt1 + dt2) / 2.0)).abs();
                if jerk > max_jerk {
                    max_jerk = jerk;
                }
            }
        }

        max_jerk
    }

    fn estimate_manufacturing_cost(
        &self,
        report: &ToleranceReport,
        profile: &[CamPoint],
    ) -> f64 {
        let mut cost = self.manufacturing_cost_base;

        if !report.curvature_ok {
            cost *= 1.5;
        }

        let tightness = (5e-5 / self.dimensional_tolerance.max(1e-9)).max(1.0);
        cost *= tightness;

        let roughness_factor = (1.6e-6 / self.surface_roughness.max(0.1e-6)).max(1.0);
        cost *= roughness_factor.sqrt();

        if !report.jerk_ok {
            cost *= 1.3;
        }

        let complexity = profile.len() as f64 / 360.0;
        cost *= 1.0 + complexity * 0.2;

        cost
    }
}

pub struct PoundingOptimizer {
    device: DeviceInfo,
    tolerance: ToleranceAnalysis,
    dynamics_config: DynamicsConfig,
}

impl PoundingOptimizer {
    pub fn new(device: DeviceInfo) -> Self {
        PoundingOptimizer {
            device,
            tolerance: ToleranceAnalysis::default(),
            dynamics_config: DynamicsConfig::load_default(),
        }
    }

    pub fn with_tolerance(device: DeviceInfo, tolerance: ToleranceAnalysis) -> Self {
        PoundingOptimizer {
            device,
            tolerance,
            dynamics_config: DynamicsConfig::load_default(),
        }
    }

    pub fn with_config(device: DeviceInfo, tolerance: ToleranceAnalysis, dynamics_config: DynamicsConfig) -> Self {
        PoundingOptimizer {
            device,
            tolerance,
            dynamics_config,
        }
    }

    pub fn optimize(&self, request: &OptimizationRequest) -> OptimizationResult {
        let constraints = &request.constraints;

        let mut best_score = 0.0;
        let mut best_params = (self.device.cam_base_radius, self.device.cam_lift, "cycloidal".to_string());
        let mut best_profile: Vec<CamPoint> = Vec::new();
        let mut best_tolerance_report = ToleranceReport::default();

        let profile_types = vec!["cycloidal", "harmonic", "trapezoidal", "polynomial"];

        let radius_step = ((constraints.max_cam_radius - constraints.min_cam_radius) / 10.0).max(0.005);
        let lift_step = (constraints.max_lift / 10.0).max(0.005);

        for profile_type in &profile_types {
            let mut r = constraints.min_cam_radius;
            while r <= constraints.max_cam_radius {
                let mut h = lift_step;
                while h <= constraints.max_lift {
                    if let Some(pressure_angle) = self.calculate_pressure_angle(r, h) {
                        if pressure_angle <= constraints.max_pressure_angle {
                            let profile = self.generate_profile(profile_type, r, h);
                            let tolerance_report = self.tolerance.analyze(&profile);

                            let efficiency = self.evaluate_efficiency(
                                &profile,
                                &request.grain_type,
                                r,
                                h,
                            );

                            let cost_factor =
                                (MANUFACTURING_COST_BASE / tolerance_report.manufacturing_cost)
                                    .max(0.3)
                                    .min(1.0);

                            let tolerance_factor = tolerance_report.overall_feasibility.max(0.3);
                            let score = efficiency * cost_factor * tolerance_factor * 0.7
                                + efficiency * 0.3;

                            if score > best_score {
                                best_score = score;
                                best_params = (r, h, profile_type.to_string());
                                best_profile = profile.clone();
                                best_tolerance_report = tolerance_report.clone();
                            }
                        }
                    }
                    h += lift_step;
                }
                r += radius_step;
            }
        }

        let avg_force = self.calculate_average_pounding_force(&best_profile, best_params.0, best_params.1);
        let impact_energy = self.calculate_impact_energy_per_cycle(best_params.1);
        let husking_rate = calculate_husking_rate(impact_energy, &request.grain_type, &self.dynamics_config);
        let breakage_rate = calculate_grain_breakage_rate(impact_energy, avg_force, &self.dynamics_config);
        let pressure_angle = self.calculate_pressure_angle(best_params.0, best_params.1)
            .unwrap_or(0.0);

        OptimizationResult {
            optimization_id: Uuid::new_v4().to_string(),
            device_id: request.device_id.clone(),
            timestamp: Utc::now(),
            grain_type: request.grain_type.clone(),
            base_radius: best_params.0,
            lift: best_params.1,
            cam_profile_type: best_params.2,
            overall_efficiency: best_score,
            husking_rate,
            breakage_rate,
            pounding_force: avg_force,
            impact_energy,
            manufacturing_cost: best_tolerance_report.manufacturing_cost,
            cam_profile: best_profile,
            tolerance_report: Some(best_tolerance_report),
            optimization_score: best_score,
        }
    }

    pub fn generate_profile(&self, profile_type: &str, base_radius: f64, lift: f64) -> Vec<CamPoint> {
        let num_points = 360;
        let mut points = Vec::with_capacity(num_points);

        for i in 0..num_points {
            let angle_rad = 2.0 * std::f64::consts::PI * i as f64 / num_points as f64;
            let angle_deg = angle_rad.to_degrees();

            let (lift_val, velocity, acceleration) = match profile_type {
                "cycloidal" => self.cycloidal_motion(angle_rad, lift),
                "harmonic" => self.harmonic_motion(angle_rad, lift),
                "trapezoidal" => self.trapezoidal_motion(angle_rad, lift),
                "polynomial" => self.polynomial_motion(angle_rad, lift),
                _ => self.harmonic_motion(angle_rad, lift),
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

    fn cycloidal_motion(&self, angle: f64, total_lift: f64) -> (f64, f64, f64) {
        let pi = std::f64::consts::PI;

        if angle < pi {
            let t = angle / pi;
            let s = total_lift * (t - (2.0 * pi * t).sin() / (2.0 * pi));
            let v = total_lift * (1.0 - (2.0 * pi * t).cos()) / pi;
            let a = 2.0 * total_lift * (2.0 * pi * t).sin();
            (s, v, a)
        } else {
            let t = (angle - pi) / pi;
            let s = total_lift * (1.0 - t + (2.0 * pi * t).sin() / (2.0 * pi));
            let v = -total_lift * (1.0 - (2.0 * pi * t).cos()) / pi;
            let a = -2.0 * total_lift * (2.0 * pi * t).sin();
            (s, v, a)
        }
    }

    fn harmonic_motion(&self, angle: f64, total_lift: f64) -> (f64, f64, f64) {
        let pi = std::f64::consts::PI;

        if angle < pi {
            let t = angle / pi;
            let s = total_lift * (1.0 - (pi * t).cos()) / 2.0;
            let v = total_lift * pi * (pi * t).sin() / 2.0;
            let a = total_lift * pi * pi * (pi * t).cos() / 2.0;
            (s, v, a)
        } else {
            let t = (angle - pi) / pi;
            let s = total_lift * (1.0 + (pi * t).cos()) / 2.0;
            let v = -total_lift * pi * (pi * t).sin() / 2.0;
            let a = -total_lift * pi * pi * (pi * t).cos() / 2.0;
            (s, v, a)
        }
    }

    fn trapezoidal_motion(&self, angle: f64, total_lift: f64) -> (f64, f64, f64) {
        let pi = std::f64::consts::PI;

        if angle < pi {
            let t = angle / pi;
            let t1 = 0.2;
            let t2 = 0.8;
            let v_max = total_lift / (pi * (t2 - t1 + t1));

            if t < t1 {
                let a_val = v_max / (t1 * pi);
                let s_val = 0.5 * a_val * (t * pi).powi(2);
                let v_val = a_val * t * pi;
                (s_val, v_val, a_val)
            } else if t < t2 {
                let s_val = 0.5 * v_max * t1 * pi + v_max * (t - t1) * pi;
                (s_val, v_max, 0.0)
            } else {
                let a_val = -v_max / ((1.0 - t2) * pi);
                let dt = (t - t2) * pi;
                let s_val = total_lift - 0.5 * (-a_val) * dt.powi(2);
                let v_val = v_max + a_val * dt;
                (s_val, v_val, a_val)
            }
        } else {
            let t = (angle - pi) / pi;
            let t1 = 0.2;
            let t2 = 0.8;
            let v_max = -total_lift / (pi * (t2 - t1 + t1));

            if t < t1 {
                let a_val = v_max / (t1 * pi);
                let s_val = total_lift + 0.5 * a_val * (t * pi).powi(2);
                let v_val = a_val * t * pi;
                (s_val, v_val, a_val)
            } else if t < t2 {
                let s_val = total_lift + 0.5 * v_max * t1 * pi + v_max * (t - t1) * pi;
                (s_val, v_max, 0.0)
            } else {
                let a_val = -v_max / ((1.0 - t2) * pi);
                let dt = (t - t2) * pi;
                let s_val = 0.0 - 0.5 * (-a_val) * dt.powi(2);
                let v_val = v_max + a_val * dt;
                (s_val, v_val, a_val)
            }
        }
    }

    fn polynomial_motion(&self, angle: f64, total_lift: f64) -> (f64, f64, f64) {
        let pi = std::f64::consts::PI;

        if angle < pi {
            let t = angle / pi;
            let s = total_lift * (10.0 * t.powi(3) - 15.0 * t.powi(4) + 6.0 * t.powi(5));
            let v = total_lift / pi * (30.0 * t.powi(2) - 60.0 * t.powi(3) + 30.0 * t.powi(4));
            let a = total_lift / (pi * pi) * (60.0 * t - 180.0 * t.powi(2) + 120.0 * t.powi(3));
            (s, v, a)
        } else {
            let t = (angle - pi) / pi;
            let s = total_lift * (1.0 - 10.0 * t.powi(3) + 15.0 * t.powi(4) - 6.0 * t.powi(5));
            let v = -total_lift / pi * (30.0 * t.powi(2) - 60.0 * t.powi(3) + 30.0 * t.powi(4));
            let a = -total_lift / (pi * pi) * (60.0 * t - 180.0 * t.powi(2) + 120.0 * t.powi(3));
            (s, v, a)
        }
    }

    pub fn evaluate_efficiency(
        &self,
        profile: &[CamPoint],
        grain_type: &str,
        base_radius: f64,
        lift: f64,
    ) -> f64 {
        let impact_energy = self.calculate_impact_energy_per_cycle(lift);
        let avg_force = self.calculate_average_pounding_force(profile, base_radius, lift);

        let husking_rate = calculate_husking_rate(impact_energy, grain_type, &self.dynamics_config);
        let breakage_rate = calculate_grain_breakage_rate(impact_energy, avg_force, &self.dynamics_config);

        let net_efficiency = husking_rate * (1.0 - breakage_rate);

        let pressure_angle = self.calculate_pressure_angle(base_radius, lift)
            .unwrap_or(std::f64::MAX);
        let pressure_penalty = if pressure_angle > 30.0_f64.to_radians() {
            (30.0_f64.to_radians() / pressure_angle).powi(2)
        } else {
            1.0
        };

        net_efficiency * pressure_penalty
    }

    fn calculate_pressure_angle(&self, base_radius: f64, lift: f64) -> Option<f64> {
        if base_radius <= 0.0 {
            return None;
        }

        let max_slope = std::f64::consts::PI * lift / 2.0;
        let pressure_angle = (max_slope / base_radius).atan();

        Some(pressure_angle)
    }

    pub fn calculate_average_pounding_force(
        &self,
        profile: &[CamPoint],
        _base_radius: f64,
        _lift: f64,
    ) -> f64 {
        let mass = self.device.duitou_mass;

        let max_accel = profile
            .iter()
            .map(|p| p.acceleration.abs())
            .fold(0.0, f64::max);

        let inertia_force = mass * max_accel;
        let gravity_force = mass * GRAVITY;

        gravity_force + inertia_force
    }

    pub fn calculate_impact_energy_per_cycle(&self, lift: f64) -> f64 {
        let mass = self.device.duitou_mass;
        let velocity = (2.0 * GRAVITY * lift).sqrt();
        0.5 * mass * velocity.powi(2)
    }
}

pub fn calculate_efficiency_score(husking_rate: f64, breakage_rate: f64) -> f64 {
    husking_rate * 0.7 + (1.0 - breakage_rate) * 0.3
}

// ============ 新增凸轮曲线类型 ============

impl PoundingOptimizer {
    pub fn involute_motion(&self, angle: f64, total_lift: f64) -> (f64, f64, f64) {
        let pi = std::f64::consts::PI;
        let base_radius = 0.1;

        if angle < pi {
            let t = angle / pi;
            let theta = t * pi;
            let involute_x = base_radius * (theta.cos() + theta * theta.sin());
            let involute_y = base_radius * (theta.sin() - theta * theta.cos());

            let s = ((involute_x - base_radius).powi(2) + involute_y.powi(2)).sqrt();
            let s_norm = (s / base_radius).min(1.0) * total_lift;

            let r = base_radius * (1.0 + theta.powi(2)).sqrt();
            let v = base_radius * theta * theta.sin() * pi;
            let a = base_radius * (theta.sin() + theta * theta.cos()) * pi * pi;

            (s_norm, v, a)
        } else {
            let t = (angle - pi) / pi;
            let theta = (1.0 - t) * pi;
            let involute_x = base_radius * (theta.cos() + theta * theta.sin());
            let involute_y = base_radius * (theta.sin() - theta * theta.cos());

            let s = ((involute_x - base_radius).powi(2) + involute_y.powi(2)).sqrt();
            let s_norm = (s / base_radius).min(1.0) * total_lift;

            let v = -base_radius * theta * theta.sin() * pi;
            let a = -base_radius * (theta.sin() + theta * theta.cos()) * pi * pi;

            (s_norm, v, a)
        }
    }

    pub fn circular_arc_motion(&self, angle: f64, total_lift: f64) -> (f64, f64, f64) {
        let pi = std::f64::consts::PI;
        let arc_radius = total_lift * 1.5;
        let center_offset = arc_radius - total_lift / 2.0;

        if angle < pi {
            let t = angle / pi;
            let arc_angle = t * pi - pi / 2.0;

            let y = arc_radius * arc_angle.sin() + center_offset;
            let s = (y + total_lift / 2.0).max(0.0).min(total_lift);

            let v = arc_radius * arc_angle.cos() * pi;
            let a = -arc_radius * arc_angle.sin() * pi * pi;

            (s, v, a)
        } else {
            let t = (angle - pi) / pi;
            let arc_angle = (1.0 - t) * pi - pi / 2.0;

            let y = arc_radius * arc_angle.sin() + center_offset;
            let s = (y + total_lift / 2.0).max(0.0).min(total_lift);

            let v = -arc_radius * arc_angle.cos() * pi;
            let a = arc_radius * arc_angle.sin() * pi * pi;

            (s, v, a)
        }
    }

    pub fn generate_profile_extended(
        &self,
        profile_type: &str,
        base_radius: f64,
        lift: f64,
    ) -> Vec<CamPoint> {
        let num_points = 360;
        let mut points = Vec::with_capacity(num_points);

        for i in 0..num_points {
            let angle_rad = 2.0 * std::f64::consts::PI * i as f64 / num_points as f64;
            let angle_deg = angle_rad.to_degrees();

            let (lift_val, velocity, acceleration) = match profile_type {
                "cycloidal" => self.cycloidal_motion(angle_rad, lift),
                "harmonic" => self.harmonic_motion(angle_rad, lift),
                "trapezoidal" => self.trapezoidal_motion(angle_rad, lift),
                "polynomial" => self.polynomial_motion(angle_rad, lift),
                "involute" => self.involute_motion(angle_rad, lift),
                "circular_arc" => self.circular_arc_motion(angle_rad, lift),
                _ => self.harmonic_motion(angle_rad, lift),
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

    pub fn generate_profile_from_user_lifts(
        &self,
        user_lifts: &[f64],
        base_radius: f64,
    ) -> Vec<CamPoint> {
        let num_points = user_lifts.len();
        let mut points = Vec::with_capacity(num_points);

        for i in 0..num_points {
            let angle_rad = 2.0 * std::f64::consts::PI * i as f64 / num_points as f64;
            let angle_deg = angle_rad.to_degrees();

            let lift_val = user_lifts[i].max(0.0);

            let (velocity, acceleration) = if num_points >= 3 {
                let prev_i = (i + num_points - 1) % num_points;
                let next_i = (i + 1) % num_points;
                let dt = 2.0 * std::f64::consts::PI / num_points as f64;

                let v = (user_lifts[next_i] - user_lifts[prev_i]) / (2.0 * dt);
                let a = (user_lifts[next_i] - 2.0 * user_lifts[i] + user_lifts[prev_i]) / (dt * dt);
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

    pub fn get_profile_name_cn(&self, profile_type: &str) -> &'static str {
        match profile_type {
            "cycloidal" => "摆线凸轮",
            "harmonic" => "简谐凸轮",
            "trapezoidal" => "梯形加速度凸轮",
            "polynomial" => "3-4-5多项式凸轮",
            "involute" => "渐开线凸轮",
            "circular_arc" => "圆弧凸轮",
            _ => "未知类型",
        }
    }
}

// ============ 功能1：多凸轮效率对比 ============

impl PoundingOptimizer {
    pub fn compare_cam_profiles(
        &self,
        request: &CamProfileComparisonRequest,
    ) -> CamProfileComparisonResult {
        let comparator = CamComparator::new(
            self.tolerance.clone(),
            self.dynamics_config.clone(),
        );
        comparator.compare_profiles(request, &self.device)
    }
}

// ============ 功能2：跨时代效率对比 ============

impl PoundingOptimizer {
    pub fn compare_cross_era(
        &self,
        request: &CrossEraComparisonRequest,
    ) -> CrossEraComparisonResult {
        let comparator = EraComparator::new(
            self.tolerance.clone(),
            self.dynamics_config.clone(),
        );
        comparator.compare_cross_era(request)
    }
}

// ============ 功能3：多台水碓振动干涉分析 ============

// ============ 功能4：公众虚拟凸轮设计体验 ============

impl PoundingOptimizer {
    pub fn test_user_cam_design(
        &self,
        request: &UserCamDesignRequest,
    ) -> UserCamDesignResult {
        let designer = VrCamDesigner::new(
            self.tolerance.clone(),
            self.dynamics_config.clone(),
        );
        designer.test_user_cam_design(request, &self.device)
    }
}

pub fn generate_profile(profile_type: &str, base_radius: f64, lift: f64) -> Vec<CamPoint> {
    let device = DeviceInfo {
        device_id: String::new(),
        device_name: String::new(),
        location: String::new(),
        cam_base_radius: base_radius,
        cam_lift: lift,
        duitou_mass: 25.0,
        water_flow_rate: 0.05,
        frame_vibration_threshold: 5.0,
    };
    let optimizer = PoundingOptimizer::new(device);
    optimizer.generate_profile(profile_type, base_radius, lift)
}

pub fn evaluate_efficiency(
    profile: &[CamPoint],
    grain_type: &str,
    base_radius: f64,
    lift: f64,
) -> f64 {
    let device = DeviceInfo {
        device_id: String::new(),
        device_name: String::new(),
        location: String::new(),
        cam_base_radius: base_radius,
        cam_lift: lift,
        duitou_mass: 25.0,
        water_flow_rate: 0.05,
        frame_vibration_threshold: 5.0,
    };
    let optimizer = PoundingOptimizer::new(device);
    optimizer.evaluate_efficiency(profile, grain_type, base_radius, lift)
}

pub fn calculate_average_pounding_force(
    profile: &[CamPoint],
    base_radius: f64,
    lift: f64,
) -> f64 {
    let device = DeviceInfo {
        device_id: String::new(),
        device_name: String::new(),
        location: String::new(),
        cam_base_radius: base_radius,
        cam_lift: lift,
        duitou_mass: 25.0,
        water_flow_rate: 0.05,
        frame_vibration_threshold: 5.0,
    };
    let optimizer = PoundingOptimizer::new(device);
    optimizer.calculate_average_pounding_force(profile, base_radius, lift)
}

pub fn calculate_impact_energy_per_cycle(lift: f64) -> f64 {
    let device = DeviceInfo {
        device_id: String::new(),
        device_name: String::new(),
        location: String::new(),
        cam_base_radius: 0.15,
        cam_lift: lift,
        duitou_mass: 25.0,
        water_flow_rate: 0.05,
        frame_vibration_threshold: 5.0,
    };
    let optimizer = PoundingOptimizer::new(device);
    optimizer.calculate_impact_energy_per_cycle(lift)
}

pub fn create_test_device() -> DeviceInfo {
    DeviceInfo {
        device_id: "test-001".to_string(),
        device_name: "Test".to_string(),
        location: "Test".to_string(),
        cam_base_radius: 0.15,
        cam_lift: 0.12,
        duitou_mass: 25.0,
        water_flow_rate: 0.05,
        frame_vibration_threshold: 5.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_device() -> DeviceInfo {
        DeviceInfo {
            device_id: "test-001".to_string(),
            device_name: "Test".to_string(),
            location: "Test".to_string(),
            cam_base_radius: 0.15,
            cam_lift: 0.12,
            duitou_mass: 25.0,
            water_flow_rate: 0.05,
            frame_vibration_threshold: 5.0,
        }
    }

    #[test]
    fn test_tolerance_analysis() {
        let device = create_test_device();
        let optimizer = PoundingOptimizer::new(device);
        let profile = optimizer.generate_profile("cycloidal", 0.15, 0.12);

        let tolerance = ToleranceAnalysis::default();
        let report = tolerance.analyze(&profile);

        assert!(report.min_curvature > 0.0);
        assert!(report.overall_feasibility >= 0.0);
        assert!(report.overall_feasibility <= 1.0);
        assert!(report.manufacturing_cost > 0.0);
    }

    #[test]
    fn test_curvature_calculation() {
        let tolerance = ToleranceAnalysis::default();
        let c = ToleranceAnalysis::curvature_from_points(
            -1.0, 0.0,
            0.0, 1.0,
            1.0, 0.0,
        );
        assert!(c > 0.0);
    }

    #[test]
    fn test_optimization() {
        let device = create_test_device();
        let optimizer = PoundingOptimizer::new(device);

        let request = OptimizationRequest {
            device_id: "test-001".to_string(),
            target_efficiency: 0.85,
            grain_type: "rice".to_string(),
            constraints: crate::models::OptimizationConstraints {
                max_cam_radius: 0.25,
                min_cam_radius: 0.1,
                max_lift: 0.2,
                max_pressure_angle: 45.0_f64.to_radians(),
            },
        };

        let result = optimizer.optimize(&request);
        assert!(result.overall_efficiency > 0.0);
        assert!(result.overall_efficiency <= 1.0);
        assert!(!result.cam_profile.is_empty());
    }

    #[test]
    fn test_profile_generation() {
        let device = create_test_device();
        let optimizer = PoundingOptimizer::new(device);

        let profile = optimizer.generate_profile("cycloidal", 0.15, 0.12);
        assert_eq!(profile.len(), 360);

        let max_lift = profile.iter().map(|p| p.lift).fold(0.0, f64::max);
        assert!((max_lift - 0.12).abs() < 0.001);
    }

    #[test]
    fn test_involute_cam_profile() {
        let device = create_test_device();
        let optimizer = PoundingOptimizer::new(device);

        let profile = optimizer.generate_profile_extended("involute", 0.15, 0.12);
        assert_eq!(profile.len(), 360);

        let max_lift = profile.iter().map(|p| p.lift).fold(0.0, f64::max);
        assert!(max_lift > 0.0);
        assert!(max_lift <= 0.12 + 0.001);
    }

    #[test]
    fn test_circular_arc_cam_profile() {
        let device = create_test_device();
        let optimizer = PoundingOptimizer::new(device);

        let profile = optimizer.generate_profile_extended("circular_arc", 0.15, 0.12);
        assert_eq!(profile.len(), 360);

        let max_lift = profile.iter().map(|p| p.lift).fold(0.0, f64::max);
        assert!(max_lift > 0.0);
        assert!(max_lift <= 0.12 + 0.001);
    }

    #[test]
    fn test_user_defined_profile() {
        let device = create_test_device();
        let optimizer = PoundingOptimizer::new(device);

        let lifts: Vec<f64> = (0..72).map(|i| {
            let t = i as f64 / 72.0;
            0.12 * (1.0 - (std::f64::consts::PI * t).cos()) / 2.0
        }).collect();

        let profile = optimizer.generate_profile_from_user_lifts(&lifts, 0.15);
        assert_eq!(profile.len(), 72);

        let max_lift = profile.iter().map(|p| p.lift).fold(0.0, f64::max);
        assert!(max_lift > 0.1);
    }

    #[test]
    fn test_cam_profile_comparison() {
        let device = create_test_device();
        let optimizer = PoundingOptimizer::new(device);

        let request = crate::models::CamProfileComparisonRequest {
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
        };

        let result = optimizer.compare_cam_profiles(&request);
        assert_eq!(result.results.len(), 4);
        assert!(!result.best_profile.is_empty());

        for r in &result.results {
            assert!(r.overall_efficiency > 0.0);
            assert!(r.overall_efficiency <= 1.0);
            assert!(!r.profile_name_cn.is_empty());
        }
    }

    #[test]
    fn test_cross_era_comparison() {
        let device = create_test_device();
        let optimizer = PoundingOptimizer::new(device);

        let request = crate::models::CrossEraComparisonRequest {
            ancient_device_id: "test-001".to_string(),
            grain_type: "rice".to_string(),
            modern_motor_power_kw: 2.2,
            modern_motor_rpm: 1450.0,
        };

        let result = optimizer.compare_cross_era(&request);
        assert!(result.efficiency_ratio > 0.0);
        assert!(result.productivity_ratio > 0.0);
        assert!(result.energy_ratio > 0.0);
        assert_eq!(result.ancient.era, "ancient");
        assert_eq!(result.modern.era, "modern");
        assert!(result.modern.pounding_rate_kg_h > result.ancient.pounding_rate_kg_h);
    }

    #[test]
    fn test_vibration_interference_analysis() {
        let device1 = create_test_device();
        let device2 = DeviceInfo {
            device_id: "test-002".to_string(),
            ..create_test_device()
        };

        let devices = vec![
            (device1, 0.0),
            (device2, std::f64::consts::PI / 2.0),
        ];

        let analyzer = VibrationInterferenceAnalyzer::new(devices);
        let result = analyzer.analyze(5.0, 0.01);

        assert!(!result.time_series.is_empty());
        assert!(result.max_interference >= 0.0);
        assert!(result.avg_interference >= 0.0);
        assert!(!result.safety_level.is_empty());
        assert!(!result.recommendation.is_empty());
    }

    #[test]
    fn test_user_cam_design() {
        let device = create_test_device();
        let optimizer = PoundingOptimizer::new(device);

        let lifts: Vec<f64> = (0..72).map(|i| {
            let t = i as f64 / 72.0;
            0.12 * (1.0 - (std::f64::consts::PI * t).cos()) / 2.0
        }).collect();

        let request = crate::models::UserCamDesignRequest {
            user_id: Some("test-user".to_string()),
            design_name: Some("我的凸轮设计".to_string()),
            base_radius: 0.15,
            grain_type: "rice".to_string(),
            user_defined_lifts: lifts,
        };

        let result = optimizer.test_user_cam_design(&request);
        assert!(result.overall_efficiency > 0.0);
        assert!(result.overall_efficiency <= 1.0);
        assert!(result.husking_rate > 0.0);
        assert!(!result.grade.is_empty());
        assert!(!result.cam_profile.is_empty());
    }

    // ============================================================
    // 功能1测试：凸轮对比 - 冲击能量验证
    // ============================================================

    #[test]
    fn test_cam_comparison_impact_energy_normal() {
        let device = create_test_device();
        let duitou_mass = device.duitou_mass;
        let optimizer = PoundingOptimizer::new(device);

        let request = crate::models::CamProfileComparisonRequest {
            device_id: "test-001".to_string(),
            grain_type: "rice".to_string(),
            profile_types: vec![
                "cycloidal".to_string(),
                "harmonic".to_string(),
                "trapezoidal".to_string(),
                "polynomial".to_string(),
                "involute".to_string(),
                "circular_arc".to_string(),
            ],
            base_radius: 0.15,
            lift: 0.12,
        };

        let result = optimizer.compare_cam_profiles(&request);

        assert_eq!(result.results.len(), 6);
        for r in &result.results {
            assert!(r.impact_energy > 0.0, "{} 的冲击能量应为正值", r.profile_name_cn);
            assert!(r.impact_energy < 100.0, "冲击能量不应超过合理范围");
            assert!(r.husking_rate > 0.0 && r.husking_rate <= 1.0);
            assert!(r.breakage_rate >= 0.0 && r.breakage_rate < 0.5);
            let expected_e = 0.5 * duitou_mass * (2.0 * 9.81 * 0.12);
            assert!((r.impact_energy - expected_e).abs() < 1.0,
                "冲击能量应接近理论值 m*g*h = {:.2}J, 实际为 {:.2}J", expected_e, r.impact_energy);
        }
    }

    #[test]
    fn test_cam_comparison_impact_energy_boundary() {
        let device = create_test_device();
        let optimizer = PoundingOptimizer::new(device);

        let request_zero = crate::models::CamProfileComparisonRequest {
            device_id: "test-001".to_string(),
            grain_type: "rice".to_string(),
            profile_types: vec!["cycloidal".to_string()],
            base_radius: 0.15,
            lift: 0.0,
        };
        let result_zero = optimizer.compare_cam_profiles(&request_zero);
        assert_eq!(result_zero.results[0].impact_energy, 0.0, "升程为0时冲击能量应为0");

        let request_small = crate::models::CamProfileComparisonRequest {
            device_id: "test-001".to_string(),
            grain_type: "millet".to_string(),
            profile_types: vec!["harmonic".to_string()],
            base_radius: 0.10,
            lift: 0.01,
        };
        let result_small = optimizer.compare_cam_profiles(&request_small);
        assert!(result_small.results[0].impact_energy > 0.0);
        assert!(result_small.results[0].impact_energy < 10.0, "极小升程下能量应很小");

        let request_large = crate::models::CamProfileComparisonRequest {
            device_id: "test-001".to_string(),
            grain_type: "wheat".to_string(),
            profile_types: vec!["polynomial".to_string()],
            base_radius: 0.30,
            lift: 0.30,
        };
        let result_large = optimizer.compare_cam_profiles(&request_large);
        assert!(result_large.results[0].impact_energy > 50.0, "大升程应有较高能量");
    }

    #[test]
    fn test_cam_comparison_impact_energy_abnormal() {
        let device = create_test_device();
        let optimizer = PoundingOptimizer::new(device);

        let request_unknown = crate::models::CamProfileComparisonRequest {
            device_id: "test-001".to_string(),
            grain_type: "rice".to_string(),
            profile_types: vec!["unknown_type_xyz".to_string()],
            base_radius: 0.15,
            lift: 0.12,
        };
        let result_unknown = optimizer.compare_cam_profiles(&request_unknown);
        assert_eq!(result_unknown.results.len(), 1);
        assert!(result_unknown.results[0].impact_energy > 0.0,
            "未知类型应回退到默认简谐凸轮，仍产生有效能量");
        assert_eq!(result_unknown.results[0].profile_name_cn, "未知类型");

        let request_empty = crate::models::CamProfileComparisonRequest {
            device_id: "test-001".to_string(),
            grain_type: "rice".to_string(),
            profile_types: vec![],
            base_radius: 0.15,
            lift: 0.12,
        };
        let result_empty = optimizer.compare_cam_profiles(&request_empty);
        assert_eq!(result_empty.results.len(), 0, "空类型列表应返回空结果");
        assert_eq!(result_empty.best_profile, "");
    }

    // ============================================================
    // 功能2测试：跨时代对比 - 能耗验证
    // ============================================================

    #[test]
    fn test_cross_era_energy_consumption_normal() {
        let device = create_test_device();
        let optimizer = PoundingOptimizer::new(device);

        let request = crate::models::CrossEraComparisonRequest {
            ancient_device_id: "test-001".to_string(),
            grain_type: "rice".to_string(),
            modern_motor_power_kw: 2.2,
            modern_motor_rpm: 1450.0,
        };

        let result = optimizer.compare_cross_era(&request);

        assert!(result.ancient.energy_consumption_kwh_100kg > 0.0);
        assert!(result.modern.energy_consumption_kwh_100kg > 0.0);
        assert!(result.energy_ratio > 0.0, "能耗比应为正值");
        assert!(result.modern.energy_consumption_kwh_100kg < result.ancient.energy_consumption_kwh_100kg,
            "现代机器能耗应低于古代水碓");
        assert!(result.modern.pounding_rate_kg_h > result.ancient.pounding_rate_kg_h,
            "现代机器生产率应高于古代水碓");

        let energy_100kg_modern = result.modern.energy_consumption_kwh_100kg;
        assert!(energy_100kg_modern > 0.001 && energy_100kg_modern < 50.0,
            "现代舂米机每100kg能耗应在合理范围(0.001~50kWh), 实际为 {:.2}", energy_100kg_modern);
    }

    #[test]
    fn test_cross_era_energy_consumption_boundary() {
        let device = create_test_device();
        let optimizer = PoundingOptimizer::new(device);

        let request_min = crate::models::CrossEraComparisonRequest {
            ancient_device_id: "test-001".to_string(),
            grain_type: "millet".to_string(),
            modern_motor_power_kw: 0.5,
            modern_motor_rpm: 900.0,
        };
        let result_min = optimizer.compare_cross_era(&request_min);
        assert!(result_min.modern.energy_consumption_kwh_100kg > 0.0);
        assert!(result_min.modern.pounding_rate_kg_h > 0.0);

        let request_max = crate::models::CrossEraComparisonRequest {
            ancient_device_id: "test-001".to_string(),
            grain_type: "wheat".to_string(),
            modern_motor_power_kw: 15.0,
            modern_motor_rpm: 2900.0,
        };
        let result_max = optimizer.compare_cross_era(&request_max);
        assert!(result_max.modern.pounding_rate_kg_h > result_min.modern.pounding_rate_kg_h,
            "大功率电机生产率应更高");
        assert!(result_max.productivity_ratio > result_min.productivity_ratio);
    }

    #[test]
    fn test_cross_era_energy_consumption_abnormal() {
        let device = create_test_device();
        let optimizer = PoundingOptimizer::new(device);

        let request_zero_power = crate::models::CrossEraComparisonRequest {
            ancient_device_id: "test-001".to_string(),
            grain_type: "rice".to_string(),
            modern_motor_power_kw: 0.0,
            modern_motor_rpm: 1450.0,
        };
        let result_zero = optimizer.compare_cross_era(&request_zero_power);
        assert!(result_zero.modern.pounding_rate_kg_h >= 0.0, "零功率不应崩溃");
        assert!(result_zero.energy_ratio > 0.0, "零功率下能耗比应有效（避免除零）");

        let request_neg_power = crate::models::CrossEraComparisonRequest {
            ancient_device_id: "test-001".to_string(),
            grain_type: "rice".to_string(),
            modern_motor_power_kw: -2.0,
            modern_motor_rpm: 1450.0,
        };
        let result_neg = optimizer.compare_cross_era(&request_neg_power);
        assert!(result_neg.modern.pounding_rate_kg_h >= 0.0, "负功率不应崩溃");

        let request_unknown_grain = crate::models::CrossEraComparisonRequest {
            ancient_device_id: "test-001".to_string(),
            grain_type: "unknown_grain_xyz".to_string(),
            modern_motor_power_kw: 2.2,
            modern_motor_rpm: 1450.0,
        };
        let result_unknown_grain = optimizer.compare_cross_era(&request_unknown_grain);
        assert!(result_unknown_grain.ancient.husking_rate >= 0.0, "未知谷物应使用默认参数");
    }

    // ============================================================
    // 功能3测试：振动干涉 - 机架响应验证
    // ============================================================

    #[test]
    fn test_vibration_frame_response_normal() {
        let device1 = create_test_device();
        let device2 = DeviceInfo { device_id: "test-002".to_string(), ..create_test_device() };
        let device3 = DeviceInfo { device_id: "test-003".to_string(), ..create_test_device() };

        let devices = vec![
            (device1, 0.0),
            (device2, std::f64::consts::PI / 2.0),
            (device3, std::f64::consts::PI),
        ];

        let analyzer = VibrationInterferenceAnalyzer::new(devices);
        let result = analyzer.analyze(5.0, 0.01);

        assert_eq!(result.device_states.len(), 3);
        assert!(!result.time_series.is_empty());

        for ts in &result.time_series {
            assert!(ts.combined_vibration >= 0.0, "合成振动幅度应为非负");
            assert!(ts.interference_factor >= 0.0, "干涉因子应为非负");
            if ts.is_resonance {
                assert!(ts.interference_factor > 1.5, "共振点干涉因子应超过阈值1.5");
            }
        }

        assert!(result.max_interference >= result.avg_interference,
            "最大干涉不应小于平均干涉");
        assert!(result.avg_interference >= 0.0);
        assert!(matches!(result.safety_level.as_str(), "安全" | "注意" | "警告" | "危险"));
        assert!(!result.recommendation.is_empty());

        for state in &result.device_states {
            assert!(state.frequency > 0.0, "设备振动频率应为正");
            assert!(state.amplitude > 0.0, "设备振动幅度应为正");
        }
    }

    #[test]
    fn test_vibration_frame_response_boundary() {
        let device1 = create_test_device();
        let device2 = DeviceInfo { device_id: "test-002".to_string(), ..create_test_device() };

        let devices_phase = vec![
            (device1.clone(), 0.0),
            (device2.clone(), 0.0),
        ];
        let analyzer_phase = VibrationInterferenceAnalyzer::new(devices_phase);
        let result_phase = analyzer_phase.analyze(3.0, 0.01);
        assert!(result_phase.max_interference >= 0.05,
            "同相位应产生可检测的干涉（最大干涉>=0.05），实际为 {:.2}", result_phase.max_interference);

        let devices_anti = vec![
            (device1.clone(), 0.0),
            (device2.clone(), std::f64::consts::PI),
        ];
        let analyzer_anti = VibrationInterferenceAnalyzer::new(devices_anti);
        let result_anti = analyzer_anti.analyze(3.0, 0.01);
        assert!(result_anti.avg_interference >= 0.0, "反相位平均干涉应为非负");
        assert!(result_anti.max_interference >= 0.0, "反相位最大干涉应为非负");

        let devices_single = vec![(device1, 0.0)];
        let analyzer_single = VibrationInterferenceAnalyzer::new(devices_single);
        let result_single = analyzer_single.analyze(5.0, 0.01);
        assert_eq!(result_single.device_states.len(), 1);
        assert!(result_single.max_interference >= 0.0 && result_single.max_interference <= 2.0,
            "单设备干涉因子应在0~2范围");
        assert!(matches!(result_single.safety_level.as_str(),
            "安全" | "注意" | "警告" | "危险"), "单设备应返回有效安全等级");

        let devices_coarse = vec![
            (create_test_device(), 0.0),
            (DeviceInfo { device_id: "d2".to_string(), ..create_test_device() }, 0.5),
        ];
        let analyzer_coarse = VibrationInterferenceAnalyzer::new(devices_coarse);
        let result_coarse = analyzer_coarse.analyze(1.0, 0.5);
        assert!(!result_coarse.time_series.is_empty());
    }

    #[test]
    fn test_vibration_frame_response_abnormal() {
        let devices = vec![
            (create_test_device(), 0.0),
            (DeviceInfo { device_id: "d2".to_string(), ..create_test_device() }, 1.0),
        ];

        let analyzer = VibrationInterferenceAnalyzer::new(devices);

        let result_zero = analyzer.analyze(0.0, 0.01);
        assert_eq!(result_zero.time_series.len(), 0, "零时长应返回空时间序列");
        assert_eq!(result_zero.max_interference, 0.0);

        let result_neg = analyzer.analyze(-1.0, 0.01);
        assert_eq!(result_neg.time_series.len(), 0, "负时长应返回空时间序列");

        let result_large_step = analyzer.analyze(10.0, 10.0);
        assert!(!result_large_step.time_series.is_empty(), "大步长不应崩溃");

        let devices_many: Vec<_> = (0..10).map(|i| {
            (DeviceInfo {
                device_id: format!("dev-{}", i),
                ..create_test_device()
            }, i as f64 * 0.3)
        }).collect();
        let analyzer_many = VibrationInterferenceAnalyzer::new(devices_many);
        let result_many = analyzer_many.analyze(2.0, 0.05);
        assert_eq!(result_many.device_states.len(), 10, "10台设备应正确处理");
        assert!(result_many.max_interference >= 0.0);
    }

    // ============================================================
    // 功能4测试：虚拟体验 - 设计自由度验证
    // ============================================================

    #[test]
    fn test_user_design_freedom_normal() {
        let device = create_test_device();
        let optimizer = PoundingOptimizer::new(device);

        let lifts_cycloidal: Vec<f64> = (0..72).map(|i| {
            let t = i as f64 / 72.0;
            let pi = std::f64::consts::PI;
            0.12 * (t - (2.0 * pi * t).sin() / (2.0 * pi))
        }).collect();

        let request = crate::models::UserCamDesignRequest {
            user_id: Some("user-001".to_string()),
            design_name: Some("仿摆线设计".to_string()),
            base_radius: 0.15,
            grain_type: "rice".to_string(),
            user_defined_lifts: lifts_cycloidal,
        };

        let result = optimizer.test_user_cam_design(&request);

        assert_eq!(result.design_name, "仿摆线设计");
        assert!(result.overall_efficiency > 0.0 && result.overall_efficiency <= 1.0);
        assert!(result.husking_rate >= 0.0 && result.husking_rate <= 1.0);
        assert!(result.breakage_rate >= 0.0 && result.breakage_rate < 0.5);
        assert!(result.pounding_force > 0.0);
        assert!(result.impact_energy >= 0.0);
        assert!(!result.grade.is_empty());
        assert!(!result.cam_profile.is_empty());
        assert!(result.tolerance_report.overall_feasibility >= 0.0
            && result.tolerance_report.overall_feasibility <= 1.0);

        assert!(result.grade.starts_with('S')
            || result.grade.starts_with('A')
            || result.grade.starts_with('B')
            || result.grade.starts_with('C')
            || result.grade.starts_with('D'),
            "评级应从S/A/B/C/D开始，实际为: {}", result.grade);
    }

    #[test]
    fn test_user_design_freedom_boundary() {
        let device = create_test_device();
        let optimizer = PoundingOptimizer::new(device);

        let lifts_min: Vec<f64> = (0..36).map(|i| {
            let t = i as f64 / 36.0;
            0.12 * (1.0 - (std::f64::consts::PI * t).cos()) / 2.0
        }).collect();
        let req_min = crate::models::UserCamDesignRequest {
            user_id: None,
            design_name: None,
            base_radius: 0.15,
            grain_type: "millet".to_string(),
            user_defined_lifts: lifts_min,
        };
        let res_min = optimizer.test_user_cam_design(&req_min);
        assert!(!res_min.cam_profile.is_empty(), "最小36点应正常工作");
        assert_eq!(res_min.design_name, "用户设计", "未命名应使用默认名称");

        let lifts_max: Vec<f64> = (0..360).map(|i| {
            let t = i as f64 / 360.0;
            0.15 * (1.0 - (std::f64::consts::PI * t).cos()) / 2.0
        }).collect();
        let req_max = crate::models::UserCamDesignRequest {
            user_id: Some("u2".to_string()),
            design_name: Some("高密度曲线".to_string()),
            base_radius: 0.15,
            grain_type: "wheat".to_string(),
            user_defined_lifts: lifts_max,
        };
        let res_max = optimizer.test_user_cam_design(&req_max);
        assert!(!res_max.cam_profile.is_empty(), "最大360点应正常工作");

        let lifts_extreme: Vec<f64> = (0..72).map(|i| {
            if i < 36 { 0.3 } else { 0.0 }
        }).collect();
        let req_extreme = crate::models::UserCamDesignRequest {
            user_id: None,
            design_name: Some("极端阶跃曲线".to_string()),
            base_radius: 0.10,
            grain_type: "rice".to_string(),
            user_defined_lifts: lifts_extreme,
        };
        let res_extreme = optimizer.test_user_cam_design(&req_extreme);
        assert!(res_extreme.overall_efficiency > 0.0);
        assert!(!res_extreme.safety_warnings.is_empty() || !res_extreme.design_feedback.is_empty());
    }

    #[test]
    fn test_user_design_freedom_abnormal() {
        let device = create_test_device();
        let optimizer = PoundingOptimizer::new(device);

        let lifts_empty: Vec<f64> = vec![];
        let req_empty = crate::models::UserCamDesignRequest {
            user_id: None,
            design_name: Some("空曲线测试".to_string()),
            base_radius: 0.15,
            grain_type: "rice".to_string(),
            user_defined_lifts: lifts_empty,
        };
        let res_empty = optimizer.test_user_cam_design(&req_empty);
        assert!(!res_empty.cam_profile.is_empty(), "空曲线应被自动补全至36点");
        assert_eq!(res_empty.cam_profile.len(), 36);

        let lifts_very_short: Vec<f64> = vec![0.0, 0.05, 0.1];
        let req_short = crate::models::UserCamDesignRequest {
            user_id: None,
            design_name: Some("极短曲线".to_string()),
            base_radius: 0.15,
            grain_type: "rice".to_string(),
            user_defined_lifts: lifts_very_short,
        };
        let res_short = optimizer.test_user_cam_design(&req_short);
        assert_eq!(res_short.cam_profile.len(), 36, "3点应被补全至36点");

        let lifts_very_long: Vec<f64> = (0..1000).map(|i| {
            let t = i as f64 / 1000.0;
            0.12 * (1.0 - (std::f64::consts::PI * t).cos()) / 2.0
        }).collect();
        let req_long = crate::models::UserCamDesignRequest {
            user_id: None,
            design_name: Some("超长曲线".to_string()),
            base_radius: 0.15,
            grain_type: "rice".to_string(),
            user_defined_lifts: lifts_very_long,
        };
        let res_long = optimizer.test_user_cam_design(&req_long);
        assert!(res_long.cam_profile.len() <= 360, "1000点应被降采样至≤360点");

        let lifts_neg: Vec<f64> = (0..72).map(|i| {
            let t = i as f64 / 72.0;
            0.12 * (1.0 - (std::f64::consts::PI * t).cos()) / 2.0 - 0.05
        }).collect();
        let req_neg = crate::models::UserCamDesignRequest {
            user_id: None,
            design_name: Some("含负值曲线".to_string()),
            base_radius: 0.15,
            grain_type: "rice".to_string(),
            user_defined_lifts: lifts_neg,
        };
        let res_neg = optimizer.test_user_cam_design(&req_neg);
        for p in &res_neg.cam_profile {
            assert!(p.lift >= 0.0, "负升程应被修正为非负，实际为 {}", p.lift);
        }

        let lifts_zero: Vec<f64> = vec![0.0; 72];
        let req_zero = crate::models::UserCamDesignRequest {
            user_id: Some("u-zero".to_string()),
            design_name: Some("全零升程".to_string()),
            base_radius: 0.20,
            grain_type: "rice".to_string(),
            user_defined_lifts: lifts_zero,
        };
        let res_zero = optimizer.test_user_cam_design(&req_zero);
        assert!(res_zero.impact_energy == 0.0 || res_zero.impact_energy.abs() < 1e-9,
            "全零升程冲击能量应为0");
        assert!(!res_zero.safety_warnings.is_empty(), "全零升程应产生警告");
    }
}
