use crate::models::{
    OptimizationRequest, OptimizationResult, CamPoint, DeviceInfo, ToleranceReport,
};
use crate::dynamics::{calculate_husking_rate, calculate_grain_breakage_rate};
use crate::config::{OptimizationConfig, ToleranceConfig, DynamicsConfig};
use chrono::Utc;
use uuid::Uuid;

const GRAVITY: f64 = 9.81;
const MANUFACTURING_COST_BASE: f64 = 1000.0;

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

    fn generate_profile(&self, profile_type: &str, base_radius: f64, lift: f64) -> Vec<CamPoint> {
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

    fn evaluate_efficiency(
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

    fn calculate_average_pounding_force(
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

    fn calculate_impact_energy_per_cycle(&self, lift: f64) -> f64 {
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
        request: &crate::models::CamProfileComparisonRequest,
    ) -> crate::models::CamProfileComparisonResult {
        let mut results = Vec::new();

        for profile_type in &request.profile_types {
            let profile = self.generate_profile_extended(
                profile_type,
                request.base_radius,
                request.lift,
            );

            let tolerance_report = self.tolerance.analyze(&profile);

            let efficiency = self.evaluate_efficiency(
                &profile,
                &request.grain_type,
                request.base_radius,
                request.lift,
            );

            let cost_factor =
                (MANUFACTURING_COST_BASE / tolerance_report.manufacturing_cost)
                    .max(0.3)
                    .min(1.0);

            let tolerance_factor = tolerance_report.overall_feasibility.max(0.3);
            let score = efficiency * cost_factor * tolerance_factor * 0.7 + efficiency * 0.3;

            let avg_force = self.calculate_average_pounding_force(&profile, request.base_radius, request.lift);
            let impact_energy = self.calculate_impact_energy_per_cycle(request.lift);
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

            let max_pressure_angle = self.calculate_pressure_angle(request.base_radius, request.lift)
                .unwrap_or(0.0);

            results.push(crate::models::ProfileEfficiencyResult {
                profile_type: profile_type.clone(),
                profile_name_cn: self.get_profile_name_cn(profile_type).to_string(),
                overall_efficiency: efficiency,
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

        crate::models::CamProfileComparisonResult {
            comparison_id: Uuid::new_v4().to_string(),
            device_id: request.device_id.clone(),
            grain_type: request.grain_type.clone(),
            results,
            best_profile,
            timestamp: Utc::now(),
        }
    }
}

// ============ 功能2：跨时代效率对比 ============

impl PoundingOptimizer {
    pub fn compare_cross_era(
        &self,
        request: &crate::models::CrossEraComparisonRequest,
    ) -> crate::models::CrossEraComparisonResult {
        let ancient_power_kw = self.device.water_flow_rate * 1000.0 * 9.81 * 2.0 / 1000.0;
        let ancient_cycles_per_hour = 3600.0 / (2.0 * std::f64::consts::PI / self.device.water_flow_rate.max(0.01) * 10.0).max(1.0);
        let ancient_kg_per_cycle = 0.5;
        let ancient_productivity = ancient_cycles_per_hour * ancient_kg_per_cycle;
        let ancient_energy = 1.0 / ancient_productivity.max(1.0) * 100.0;

        let ancient_profile = self.generate_profile("cycloidal", self.device.cam_base_radius, self.device.cam_lift);
        let ancient_eff = self.evaluate_efficiency(
            &ancient_profile,
            &request.grain_type,
            self.device.cam_base_radius,
            self.device.cam_lift,
        );
        let ancient_impact = self.calculate_impact_energy_per_cycle(self.device.cam_lift);
        let ancient_husking = calculate_husking_rate(ancient_impact, &request.grain_type, &self.dynamics_config);
        let ancient_breakage = calculate_grain_breakage_rate(ancient_impact,
            self.calculate_average_pounding_force(&ancient_profile, self.device.cam_base_radius, self.device.cam_lift),
            &self.dynamics_config);

        let ancient = crate::models::EraMachineSpecs {
            era: "ancient".to_string(),
            name: format!("汉代水碓 ({})", self.device.device_name),
            power_source: "水力".to_string(),
            power_kw: ancient_power_kw,
            efficiency: ancient_eff,
            pounding_rate_kg_h: ancient_productivity,
            energy_consumption_kwh_100kg: ancient_energy,
            husking_rate: ancient_husking,
            breakage_rate: ancient_breakage,
            noise_db: 75.0,
            cost_cny: 5000.0,
            lifespan_years: 30.0,
        };

        let motor_rps = request.modern_motor_rpm / 60.0;
        let transmission_ratio = 30.0;
        let pounding_freq = motor_rps / transmission_ratio;
        let mechanical_eff = 0.85;
        let modern_pounding_force = 25.0 * 9.81 * 1.5;
        let modern_kg_per_cycle = 1.0;
        let modern_productivity = pounding_freq * 3600.0 * modern_kg_per_cycle;
        let modern_energy = request.modern_motor_power_kw / modern_productivity.max(1.0) * 100.0;

        let modern_husking = 0.92;
        let modern_breakage = 0.03;

        let modern = crate::models::EraMachineSpecs {
            era: "modern".to_string(),
            name: format!("现代电动舂米机 ({}kW)", request.modern_motor_power_kw),
            power_source: "电力".to_string(),
            power_kw: request.modern_motor_power_kw,
            efficiency: modern_husking * (1.0 - modern_breakage) * mechanical_eff,
            pounding_rate_kg_h: modern_productivity,
            energy_consumption_kwh_100kg: modern_energy,
            husking_rate: modern_husking,
            breakage_rate: modern_breakage,
            noise_db: 85.0,
            cost_cny: 3000.0,
            lifespan_years: 10.0,
        };

        let efficiency_ratio = modern.efficiency / ancient.efficiency.max(0.01);
        let productivity_ratio = modern.pounding_rate_kg_h / ancient.pounding_rate_kg_h.max(0.01);
        let energy_ratio = ancient.energy_consumption_kwh_100kg / modern.energy_consumption_kwh_100kg.max(0.01);

        crate::models::CrossEraComparisonResult {
            comparison_id: Uuid::new_v4().to_string(),
            grain_type: request.grain_type.clone(),
            ancient,
            modern,
            efficiency_ratio,
            productivity_ratio,
            energy_ratio,
            timestamp: Utc::now(),
        }
    }
}

// ============ 功能3：多台水碓振动干涉分析 ============

pub struct VibrationInterferenceAnalyzer {
    devices: Vec<(DeviceInfo, f64)>,
    sampling_rate: f64,
}

impl VibrationInterferenceAnalyzer {
    pub fn new(devices: Vec<(DeviceInfo, f64)>) -> Self {
        VibrationInterferenceAnalyzer {
            devices,
            sampling_rate: 100.0,
        }
    }

    pub fn analyze(
        &self,
        duration_secs: f64,
        time_step: f64,
    ) -> crate::models::VibrationInterferenceResult {
        let mut device_states = Vec::new();
        let pi = std::f64::consts::PI;

        for (i, (device, phase_offset)) in self.devices.iter().enumerate() {
            let angle = (i as f64) * 2.0 * pi / self.devices.len() as f64;
            let distance = 2.0 + (i as f64) * 0.5;
            device_states.push(crate::models::DeviceVibrationState {
                device_id: device.device_id.clone(),
                phase_offset: *phase_offset,
                position: (distance * angle.cos(), distance * angle.sin()),
                frequency: device.water_flow_rate.max(0.01) * 5.0,
                amplitude: 1.0 + device.duitou_mass / 50.0,
            });
        }

        let mut time_series = Vec::new();
        let num_steps = (duration_secs / time_step) as usize;

        let mut max_interference = 0.0;
        let mut total_interference = 0.0;
        let mut resonance_count = 0;

        for step in 0..num_steps {
            let t = step as f64 * time_step;

            let mut vib_x_total = 0.0;
            let mut vib_y_total = 0.0;
            let mut sum_amp = 0.0;

            for state in &device_states {
                let phase = 2.0 * pi * state.frequency * t + state.phase_offset;
                let vib = state.amplitude * phase.sin();

                let dx = state.position.0;
                let dy = state.position.1;
                let dist_sq = dx * dx + dy * dy;
                let attenuation = 1.0 / (1.0 + dist_sq * 0.1);

                vib_x_total += vib * dx * attenuation;
                vib_y_total += vib * dy * attenuation;
                sum_amp += state.amplitude * attenuation;
            }

            let combined = (vib_x_total.powi(2) + vib_y_total.powi(2)).sqrt();
            let interference = if sum_amp > 0.0 { combined / sum_amp } else { 0.0 };

            let is_resonance = interference > 1.5;
            if is_resonance {
                resonance_count += 1;
            }

            if interference > max_interference {
                max_interference = interference;
            }
            total_interference += interference;

            time_series.push(crate::models::InterferencePoint {
                time: t,
                x: vib_x_total,
                y: vib_y_total,
                combined_vibration: combined,
                interference_factor: interference,
                is_resonance,
            });
        }

        let avg_interference = if num_steps > 0 { total_interference / num_steps as f64 } else { 0.0 };

        let (safety_level, recommendation) = if max_interference < 0.8 {
            ("安全".to_string(), "振动干涉在安全范围内，设备可正常运行。".to_string())
        } else if max_interference < 1.2 {
            ("注意".to_string(), "存在轻度振动干涉，建议监控设备运行状态。".to_string())
        } else if max_interference < 1.8 {
            ("警告".to_string(), "振动干涉较明显，建议调整设备相位差或增加间隔距离。".to_string())
        } else {
            ("危险".to_string(), "存在严重共振风险！请立即调整设备布局或工作相位。".to_string())
        };

        crate::models::VibrationInterferenceResult {
            analysis_id: Uuid::new_v4().to_string(),
            device_states,
            time_series,
            max_interference,
            avg_interference,
            resonance_count,
            safety_level,
            recommendation,
            timestamp: Utc::now(),
        }
    }
}

// ============ 功能4：公众虚拟凸轮设计体验 ============

impl PoundingOptimizer {
    pub fn test_user_cam_design(
        &self,
        request: &crate::models::UserCamDesignRequest,
    ) -> crate::models::UserCamDesignResult {
        let mut lifts = request.user_defined_lifts.clone();

        while lifts.len() < 36 {
            let last = *lifts.last().unwrap_or(&0.0);
            lifts.push(last);
        }
        while lifts.len() > 360 {
            lifts = lifts.into_iter().step_by(2).collect();
        }

        let profile = self.generate_profile_from_user_lifts(&lifts, request.base_radius);
        let tolerance_report = self.tolerance.analyze(&profile);

        let max_lift = profile.iter().map(|p| p.lift).fold(0.0, f64::max);
        let safe_lift = max_lift.min(0.3);

        let efficiency = self.evaluate_efficiency(&profile, &request.grain_type, request.base_radius, safe_lift);
        let avg_force = self.calculate_average_pounding_force(&profile, request.base_radius, safe_lift);
        let impact_energy = self.calculate_impact_energy_per_cycle(safe_lift);
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

        crate::models::UserCamDesignResult {
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
}
