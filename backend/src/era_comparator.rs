use crate::models::{
    CrossEraComparisonRequest, CrossEraComparisonResult,
    EraMachineSpecs, DeviceInfo,
};
use crate::config::DynamicsConfig;
use crate::dynamics::{calculate_husking_rate, calculate_grain_breakage_rate};
use crate::optimization::{
    ToleranceAnalysis, calculate_average_pounding_force,
    generate_profile, evaluate_efficiency, calculate_impact_energy_per_cycle,
};

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

const ANCIENT_SHUIDUI_ARCHAEOLOGICAL: &str = "ancient";
const ANCIENT_DYNASIES: &[&str] = &["汉代", "唐代", "宋代", "元代", "明代", "清代"];

#[derive(Debug, Clone)]
pub struct AncientShuiduiSpecs {
    pub water_head_m: f64,
    pub water_flow_m3_s: f64,
    pub duitou_mass_kg: f64,
    pub cam_base_radius_m: f64,
    pub cam_lift_m: f64,
    pub cycles_per_min: f64,
    pub kg_per_cycle: f64,
    pub noise_db: f64,
    pub cost_cny_ancient: f64,
    pub lifespan_years: f64,
    pub mechanical_efficiency: f64,
    pub reference: &'static str,
}

#[derive(Debug, Clone)]
pub struct ModernRiceMillStandard {
    pub model: &'static str,
    pub power_kw: f64,
    pub capacity_kg_h: f64,
    pub husking_rate: f64,
    pub breakage_rate: f64,
    pub energy_kwh_100kg: f64,
    pub noise_db: f64,
    pub mechanical_efficiency: f64,
    pub cost_cny: f64,
    pub lifespan_years: f64,
    pub standard: &'static str,
}

pub fn get_ancient_archaeological_specs() -> AncientShuiduiSpecs {
    AncientShuiduiSpecs {
        water_head_m: 2.0,
        water_flow_m3_s: 0.05,
        duitou_mass_kg: 30.0,
        cam_base_radius_m: 0.12,
        cam_lift_m: 0.15,
        cycles_per_min: 15.0,
        kg_per_cycle: 0.04,
        noise_db: 82.0,
        cost_cny_ancient: 15000.0,
        lifespan_years: 25.0,
        mechanical_efficiency: 0.45,
        reference: "《天工开物·粹精》+ 河南巩义铁生沟汉代冶铁遗址出土水碓构件实测",
    }
}

pub fn get_modern_standard_specs(requested_power_kw: f64) -> ModernRiceMillStandard {
    if requested_power_kw <= 1.5 {
        ModernRiceMillStandard {
            model: "SM-150 家用小型",
            power_kw: 1.5,
            capacity_kg_h: 200.0,
            husking_rate: 0.92,
            breakage_rate: 0.04,
            energy_kwh_100kg: 0.90,
            noise_db: 82.0,
            mechanical_efficiency: 0.82,
            cost_cny: 1800.0,
            lifespan_years: 8.0,
            standard: "GB/T 25731-2010 粮油机械 砻谷机",
        }
    } else if requested_power_kw <= 2.5 {
        ModernRiceMillStandard {
            model: "SM-220 标准型",
            power_kw: 2.2,
            capacity_kg_h: 380.0,
            husking_rate: 0.94,
            breakage_rate: 0.035,
            energy_kwh_100kg: 0.70,
            noise_db: 85.0,
            mechanical_efficiency: 0.85,
            cost_cny: 3200.0,
            lifespan_years: 10.0,
            standard: "GB/T 25731-2010 粮油机械 砻谷机",
        }
    } else if requested_power_kw <= 4.0 {
        ModernRiceMillStandard {
            model: "SM-300 商用型",
            power_kw: 3.0,
            capacity_kg_h: 650.0,
            husking_rate: 0.95,
            breakage_rate: 0.03,
            energy_kwh_100kg: 0.55,
            noise_db: 88.0,
            mechanical_efficiency: 0.86,
            cost_cny: 5800.0,
            lifespan_years: 12.0,
            standard: "GB/T 25731-2010 粮油机械 砻谷机",
        }
    } else {
        ModernRiceMillStandard {
            model: "SM-750 工业型",
            power_kw: 7.5,
            capacity_kg_h: 1600.0,
            husking_rate: 0.96,
            breakage_rate: 0.025,
            energy_kwh_100kg: 0.45,
            noise_db: 92.0,
            mechanical_efficiency: 0.88,
            cost_cny: 12800.0,
            lifespan_years: 15.0,
            standard: "GB/T 25731-2010 粮油机械 砻谷机",
        }
    }
}

pub struct EraComparator {
    tolerance: ToleranceAnalysis,
    dynamics_config: DynamicsConfig,
}

impl EraComparator {
    pub fn new(tolerance: ToleranceAnalysis, dynamics_config: DynamicsConfig) -> Self {
        EraComparator {
            tolerance,
            dynamics_config,
        }
    }

    pub fn compare_cross_era(
        &self,
        request: &CrossEraComparisonRequest,
    ) -> CrossEraComparisonResult {
        let arch = get_ancient_archaeological_specs();

        let ancient_gravity_accel = 9.81;
        let ancient_power_kw = arch.water_flow_m3_s * 1000.0
            * ancient_gravity_accel
            * arch.water_head_m
            * arch.mechanical_efficiency
            / 1000.0;

        let ancient_cycles_per_hour = arch.cycles_per_min * 60.0;
        let ancient_productivity = ancient_cycles_per_hour * arch.kg_per_cycle;
        let ancient_energy = ancient_power_kw / ancient_productivity.max(1.0) * 100.0;

        let ancient_profile = generate_profile(
            "cycloidal",
            arch.cam_base_radius_m,
            arch.cam_lift_m,
        );
        let ancient_eff = evaluate_efficiency(
            &ancient_profile,
            &request.grain_type,
            arch.cam_base_radius_m,
            arch.cam_lift_m,
        );
        let ancient_impact = calculate_impact_energy_per_cycle(arch.cam_lift_m);
        let ancient_husking = calculate_husking_rate(
            ancient_impact,
            &request.grain_type,
            &self.dynamics_config,
        );
        let ancient_breakage = calculate_grain_breakage_rate(
            ancient_impact,
            calculate_average_pounding_force(
                &ancient_profile,
                arch.cam_base_radius_m,
                arch.cam_lift_m,
            ),
            &self.dynamics_config,
        );

        let ancient = EraMachineSpecs {
            era: ANCIENT_SHUIDUI_ARCHAEOLOGICAL.to_string(),
            name: format!(
                "古代水碓（{}实测，{}）",
                ANCIENT_DYNASIES[0],
                arch.reference
            ),
            power_source: format!(
                "水力（落差{:.1}m，流量{:.3}m³/s）",
                arch.water_head_m, arch.water_flow_m3_s
            ),
            power_kw: ancient_power_kw,
            efficiency: ancient_eff.min(1.0),
            pounding_rate_kg_h: ancient_productivity,
            energy_consumption_kwh_100kg: ancient_energy.max(0.01),
            husking_rate: ancient_husking,
            breakage_rate: ancient_breakage,
            noise_db: arch.noise_db,
            cost_cny: arch.cost_cny_ancient,
            lifespan_years: arch.lifespan_years,
        };

        let modern_std = get_modern_standard_specs(request.modern_motor_power_kw);

        let modern = EraMachineSpecs {
            era: "modern".to_string(),
            name: format!(
                "现代电动砻谷机 {}（符合{}）",
                modern_std.model, modern_std.standard
            ),
            power_source: "电力（三相异步电机）".to_string(),
            power_kw: modern_std.power_kw,
            efficiency: modern_std.husking_rate
                * (1.0 - modern_std.breakage_rate)
                * modern_std.mechanical_efficiency,
            pounding_rate_kg_h: modern_std.capacity_kg_h,
            energy_consumption_kwh_100kg: modern_std.energy_kwh_100kg,
            husking_rate: modern_std.husking_rate,
            breakage_rate: modern_std.breakage_rate,
            noise_db: modern_std.noise_db,
            cost_cny: modern_std.cost_cny,
            lifespan_years: modern_std.lifespan_years,
        };

        let efficiency_ratio = modern.efficiency / ancient.efficiency.max(0.01);
        let productivity_ratio = modern.pounding_rate_kg_h / ancient.pounding_rate_kg_h.max(0.01);
        let energy_ratio = ancient.energy_consumption_kwh_100kg / modern.energy_consumption_kwh_100kg.max(0.01);

        CrossEraComparisonResult {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::optimization::create_test_device;

    fn make_test_request() -> CrossEraComparisonRequest {
        CrossEraComparisonRequest {
            ancient_device_id: "test-001".to_string(),
            grain_type: "rice".to_string(),
            modern_motor_power_kw: 2.2,
            modern_motor_rpm: 1450.0,
        }
    }

    #[test]
    fn test_era_comparator_basic() {
        let tolerance = ToleranceAnalysis::default();
        let dynamics = DynamicsConfig::default();
        let comparator = EraComparator::new(tolerance, dynamics);

        let result = comparator.compare_cross_era(&make_test_request());
        assert_eq!(result.ancient.era, "ancient");
        assert_eq!(result.modern.era, "modern");
        assert!(result.efficiency_ratio > 0.0);
        assert!(result.productivity_ratio > 0.0);
        assert!(result.energy_ratio > 0.0);
    }

    #[test]
    fn test_era_comparator_productivity_modern_higher() {
        let tolerance = ToleranceAnalysis::default();
        let dynamics = DynamicsConfig::default();
        let comparator = EraComparator::new(tolerance, dynamics);

        let result = comparator.compare_cross_era(&make_test_request());
        assert!(
            result.modern.pounding_rate_kg_h > result.ancient.pounding_rate_kg_h,
            "现代机器生产率应高于古代水碓：现代{:.1} vs 古代{:.1}",
            result.modern.pounding_rate_kg_h, result.ancient.pounding_rate_kg_h
        );
    }

    #[test]
    fn test_era_comparator_energy_modern_lower() {
        let tolerance = ToleranceAnalysis::default();
        let dynamics = DynamicsConfig::default();
        let comparator = EraComparator::new(tolerance, dynamics);

        let result = comparator.compare_cross_era(&make_test_request());
        assert!(
            result.modern.energy_consumption_kwh_100kg < result.ancient.energy_consumption_kwh_100kg,
            "现代机器能耗应低于古代水碓：现代{:.2} vs 古代{:.2}",
            result.modern.energy_consumption_kwh_100kg, result.ancient.energy_consumption_kwh_100kg
        );
    }

    #[test]
    fn test_era_comparator_all_power_tiers() {
        let tolerance = ToleranceAnalysis::default();
        let dynamics = DynamicsConfig::default();
        let comparator = EraComparator::new(tolerance, dynamics);

        let powers = vec![1.5, 2.2, 3.0, 7.5];
        for p in powers {
            let mut req = make_test_request();
            req.modern_motor_power_kw = p;
            let result = comparator.compare_cross_era(&req);
            assert!(result.modern.pounding_rate_kg_h > 0.0);
            assert!(result.modern.energy_consumption_kwh_100kg > 0.0);
            assert!(result.modern.energy_consumption_kwh_100kg < 2.0,
                "国标机器每100kg能耗应<2kWh");
        }
    }

    #[test]
    fn test_ancient_specs_references_archaeology() {
        let specs = get_ancient_archaeological_specs();
        assert!(specs.reference.contains("天工开物"));
        assert!(specs.reference.contains("铁生沟"));
        assert_eq!(specs.cam_base_radius_m, 0.12);
        assert_eq!(specs.cam_lift_m, 0.15);
        assert_eq!(specs.cycles_per_min, 15.0);
    }

    #[test]
    fn test_modern_specs_gb_standard() {
        let specs = get_modern_standard_specs(2.2);
        assert!(specs.standard.contains("GB/T 25731-2010"));
        assert!(specs.model.contains("SM-220"));
        assert!(specs.husking_rate >= 0.92);
        assert!(specs.breakage_rate <= 0.04);
    }

    #[test]
    fn test_power_tier_ranges() {
        let s1 = get_modern_standard_specs(1.0);
        assert_eq!(s1.power_kw, 1.5);

        let s2 = get_modern_standard_specs(2.2);
        assert_eq!(s2.power_kw, 2.2);

        let s3 = get_modern_standard_specs(3.5);
        assert_eq!(s3.power_kw, 3.0);

        let s4 = get_modern_standard_specs(10.0);
        assert_eq!(s4.power_kw, 7.5);
    }

    #[test]
    fn test_capacity_scales_with_power() {
        let s1 = get_modern_standard_specs(1.0);
        let s2 = get_modern_standard_specs(2.2);
        let s3 = get_modern_standard_specs(3.0);
        let s4 = get_modern_standard_specs(10.0);

        assert!(s1.capacity_kg_h < s2.capacity_kg_h);
        assert!(s2.capacity_kg_h < s3.capacity_kg_h);
        assert!(s3.capacity_kg_h < s4.capacity_kg_h);
    }
}
