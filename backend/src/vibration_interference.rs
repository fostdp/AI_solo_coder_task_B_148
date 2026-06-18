use crate::models::{
    DeviceVibrationState, InterferencePoint,
    VibrationInterferenceResult, DeviceInfo,
    FoundationProperties, FoundationType,
};

use uuid::Uuid;
use chrono::{DateTime, Utc};

pub fn get_foundation_props_for_type(
    ftype: FoundationType,
) -> FoundationProperties {
    match ftype {
        FoundationType::Soil => FoundationProperties {
            foundation_type: ftype,
            natural_frequency_hz: 3.0,
            damping_ratio: 0.15,
            stiffness_n_m: 1.0e7,
            mass_kg: 5000.0,
            coupling_factor: 0.95,
        },
        FoundationType::ConcreteSlab => FoundationProperties {
            foundation_type: ftype,
            natural_frequency_hz: 8.0,
            damping_ratio: 0.05,
            stiffness_n_m: 5.0e7,
            mass_kg: 10000.0,
            coupling_factor: 0.70,
        },
        FoundationType::ReinforcedConcrete => FoundationProperties {
            foundation_type: ftype,
            natural_frequency_hz: 15.0,
            damping_ratio: 0.03,
            stiffness_n_m: 1.5e8,
            mass_kg: 25000.0,
            coupling_factor: 0.50,
        },
        FoundationType::PileFoundation => FoundationProperties {
            foundation_type: ftype,
            natural_frequency_hz: 25.0,
            damping_ratio: 0.10,
            stiffness_n_m: 5.0e8,
            mass_kg: 50000.0,
            coupling_factor: 0.30,
        },
    }
}

pub fn foundation_transmissibility(
    forcing_freq_hz: f64,
    foundation: &FoundationProperties,
) -> f64 {
    let r = forcing_freq_hz / foundation.natural_frequency_hz.max(0.1);
    let zeta = foundation.damping_ratio;
    let numerator = (1.0 + (2.0 * zeta * r).powi(2)).sqrt();
    let denominator = ((1.0 - r.powi(2)).powi(2) + (2.0 * zeta * r).powi(2)).sqrt();
    numerator / denominator.max(0.001)
}

pub struct VibrationInterferenceAnalyzer {
    devices: Vec<(DeviceInfo, f64)>,
    pub sampling_rate: f64,
    pub foundation: FoundationProperties,
}

impl VibrationInterferenceAnalyzer {
    pub fn new(devices: Vec<(DeviceInfo, f64)>) -> Self {
        VibrationInterferenceAnalyzer {
            devices,
            sampling_rate: 100.0,
            foundation: FoundationProperties::default(),
        }
    }

    pub fn with_foundation(
        devices: Vec<(DeviceInfo, f64)>,
        foundation: FoundationProperties,
    ) -> Self {
        VibrationInterferenceAnalyzer {
            devices,
            sampling_rate: 100.0,
            foundation,
        }
    }

    pub fn analyze(
        &self,
        duration_secs: f64,
        time_step: f64,
    ) -> VibrationInterferenceResult {
        let mut device_states = Vec::new();
        let pi = std::f64::consts::PI;

        for (i, (device, phase_offset)) in self.devices.iter().enumerate() {
            let angle = (i as f64) * 2.0 * pi / self.devices.len() as f64;
            let distance = 2.0 + (i as f64) * 0.5;
            let frequency = device.water_flow_rate.max(0.01) * 5.0;
            let amplitude = 1.0 + device.duitou_mass / 50.0;
            let trans = foundation_transmissibility(frequency, &self.foundation);
            let foundation_transmitted_amp = amplitude * trans * self.foundation.coupling_factor;

            device_states.push(DeviceVibrationState {
                device_id: device.device_id.clone(),
                phase_offset: *phase_offset,
                position: (distance * angle.cos(), distance * angle.sin()),
                frequency,
                amplitude,
                foundation_transmitted_amp,
            });
        }

        let mut time_series = Vec::new();
        let num_steps = (duration_secs / time_step) as usize;

        let mut max_interference = 0.0;
        let mut total_interference = 0.0;
        let mut max_foundation_vib = 0.0;
        let mut resonance_count = 0;
        let mut foundation_resonance_count = 0;

        for step in 0..num_steps {
            let t = step as f64 * time_step;

            let mut vib_x_total = 0.0;
            let mut vib_y_total = 0.0;
            let mut foundation_vib = 0.0;
            let mut sum_amp = 0.0;
            let mut sum_foundation_amp = 0.0;

            for state in &device_states {
                let phase = 2.0 * pi * state.frequency * t + state.phase_offset;
                let vib = state.amplitude * phase.sin();

                let dx = state.position.0;
                let dy = state.position.1;
                let dist_sq = dx * dx + dy * dy;
                let spatial_attenuation = 1.0 / (1.0 + dist_sq * 0.1);

                vib_x_total += vib * dx * spatial_attenuation;
                vib_y_total += vib * dy * spatial_attenuation;
                sum_amp += state.amplitude * spatial_attenuation;

                let foundation_phase = phase;
                foundation_vib += state.foundation_transmitted_amp
                    * foundation_phase.sin()
                    * spatial_attenuation;
                sum_foundation_amp += state.foundation_transmitted_amp * spatial_attenuation;
            }

            let combined = (vib_x_total.powi(2) + vib_y_total.powi(2)).sqrt();
            let interference = if sum_amp > 0.0 { combined / sum_amp } else { 0.0 };
            let foundation_abs = foundation_vib.abs();
            let foundation_norm = if sum_foundation_amp > 0.0 {
                foundation_abs / sum_foundation_amp
            } else {
                0.0
            };

            let is_resonance = interference > 1.5;
            let is_foundation_coupled = foundation_norm > 0.6;

            if is_resonance {
                resonance_count += 1;
            }
            if foundation_norm > 0.8 {
                foundation_resonance_count += 1;
            }

            if interference > max_interference {
                max_interference = interference;
            }
            if foundation_abs > max_foundation_vib {
                max_foundation_vib = foundation_abs;
            }
            total_interference += interference;

            time_series.push(InterferencePoint {
                time: t,
                x: vib_x_total,
                y: vib_y_total,
                combined_vibration: combined,
                foundation_vibration: foundation_abs,
                interference_factor: interference,
                is_resonance,
                is_foundation_coupled,
            });
        }

        let avg_interference = if num_steps > 0 { total_interference / num_steps as f64 } else { 0.0 };

        let foundation_risk =
            max_foundation_vib * self.foundation.coupling_factor;
        let combined_risk = max_interference * 0.6 + foundation_risk * 0.4;

        let (safety_level, recommendation) = if combined_risk < 0.8 {
            (
                "安全".to_string(),
                format!(
                    "振动干涉与地基耦合均在安全范围内，地基类型{:?}，耦合系数{:.2}。设备可正常运行。",
                    self.foundation.foundation_type, self.foundation.coupling_factor
                ),
            )
        } else if combined_risk < 1.2 {
            (
                "注意".to_string(),
                format!(
                    "存在轻度振动干涉，最大地基振动{:.3}。建议监控设备运行状态，关注地基耦合效应。",
                    max_foundation_vib
                ),
            )
        } else if combined_risk < 1.8 {
            (
                "警告".to_string(),
                format!(
                    "振动干涉较明显且地基耦合增强。建议调整设备相位差、增加间隔距离或考虑采用钢筋混凝土基础。"
                ),
            )
        } else {
            (
                "危险".to_string(),
                format!(
                    "存在严重共振与地基耦合风险！最大干涉{:.2}，地基耦合放大{:.2}倍。请立即调整布局、相位或更换桩基础。",
                    max_interference,
                    max_foundation_vib.max(1.0)
                ),
            )
        };

        VibrationInterferenceResult {
            analysis_id: Uuid::new_v4().to_string(),
            device_states,
            time_series,
            foundation: self.foundation.clone(),
            max_interference,
            avg_interference,
            max_foundation_vibration: max_foundation_vib,
            resonance_count,
            foundation_resonance_count,
            safety_level,
            recommendation,
            timestamp: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::optimization::create_test_device;

    fn make_analyzer(n: usize) -> VibrationInterferenceAnalyzer {
        let mut devices = Vec::new();
        for i in 0..n {
            let device = DeviceInfo {
                device_id: format!("test-{:03}", i),
                ..create_test_device()
            };
            let phase = (i as f64) * 2.0 * std::f64::consts::PI / n as f64;
            devices.push((device, phase));
        }
        VibrationInterferenceAnalyzer::new(devices)
    }

    #[test]
    fn test_vibration_analyzer_single_device() {
        let analyzer = make_analyzer(1);
        let result = analyzer.analyze(2.0, 0.1);
        assert_eq!(result.device_states.len(), 1);
        assert!(result.time_series.len() > 0);
        assert!(!result.safety_level.is_empty());
        assert!(matches!(
            result.safety_level.as_str(),
            "安全" | "注意" | "警告" | "危险"
        ));
    }

    #[test]
    fn test_vibration_analyzer_multi_device() {
        let analyzer = make_analyzer(4);
        let result = analyzer.analyze(5.0, 0.01);
        assert_eq!(result.device_states.len(), 4);
        assert!(result.max_interference >= 0.0);
        assert!(result.avg_interference >= 0.0);
        assert!(result.max_foundation_vibration >= 0.0);
    }

    #[test]
    fn test_foundation_coupling_soil_vs_pile() {
        let devices: Vec<_> = (0..3).map(|i| {
            let device = DeviceInfo {
                device_id: format!("test-{:03}", i),
                ..create_test_device()
            };
            (device, (i as f64) * 0.5)
        }).collect();

        let soil = VibrationInterferenceAnalyzer::with_foundation(
            devices.clone(),
            get_foundation_props_for_type(FoundationType::Soil)
        );
        let pile = VibrationInterferenceAnalyzer::with_foundation(
            devices.clone(),
            get_foundation_props_for_type(FoundationType::PileFoundation)
        );

        let r_soil = soil.analyze(3.0, 0.05);
        let r_pile = pile.analyze(3.0, 0.05);

        assert!(r_soil.foundation.coupling_factor > r_pile.foundation.coupling_factor,
            "土壤基础耦合系数应高于桩基础");
        assert!(r_soil.max_foundation_vibration >= 0.0);
        assert!(r_pile.max_foundation_vibration >= 0.0);
    }

    #[test]
    fn test_foundation_transmissibility_resonance() {
        let foundation = FoundationProperties::default();
        let trans_at_resonance = foundation_transmissibility(
            foundation.natural_frequency_hz,
            &foundation
        );
        let trans_away = foundation_transmissibility(
            foundation.natural_frequency_hz * 10.0,
            &foundation
        );
        assert!(trans_at_resonance > 1.0,
            "共振点传递率应>1.0，实际{:.2}", trans_at_resonance);
        assert!(trans_away < 1.0,
            "高频区传递率应<1.0（隔振），实际{:.2}", trans_away);
    }

    #[test]
    fn test_all_foundation_types() {
        use FoundationType::*;
        let types = vec![Soil, ConcreteSlab, ReinforcedConcrete, PileFoundation];
        for t in types {
            let props = get_foundation_props_for_type(t);
            assert!(props.natural_frequency_hz > 0.0);
            assert!(props.damping_ratio > 0.0 && props.damping_ratio < 1.0);
            assert!(props.coupling_factor > 0.0 && props.coupling_factor <= 1.0);
            assert!(props.stiffness_n_m > 0.0);
        }
    }

    #[test]
    fn test_analyzer_with_foundation() {
        let devices: Vec<_> = (0..2).map(|i| {
            let d = DeviceInfo { device_id: format!("d{}", i), ..create_test_device() };
            (d, (i as f64) * 1.0)
        }).collect();

        let foundation = get_foundation_props_for_type(FoundationType::ReinforcedConcrete);
        let analyzer = VibrationInterferenceAnalyzer::with_foundation(devices, foundation);
        let result = analyzer.analyze(2.0, 0.05);
        assert!(result.recommendation.len() > 5);
        let num_steps = (2.0 / 0.05) as u32;
        assert!(result.foundation_resonance_count >= 0 && result.foundation_resonance_count <= num_steps);
        assert!(matches!(result.foundation.foundation_type, FoundationType::ReinforcedConcrete));
    }

    #[test]
    fn test_interference_point_fields() {
        let analyzer = make_analyzer(2);
        let result = analyzer.analyze(1.0, 0.5);
        assert!(!result.time_series.is_empty());
        let pt = &result.time_series[0];
        assert!(pt.combined_vibration >= 0.0);
        assert!(pt.foundation_vibration >= 0.0);
        assert!(pt.interference_factor >= 0.0);
    }
}
