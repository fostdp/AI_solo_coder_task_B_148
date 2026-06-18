use crate::models::{SensorData, DynamicsResult, Alert, DeviceInfo, OptimizationResult};
use clickhouse::{Client, Row};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use futures_util::TryFutureExt;

#[derive(Debug, Error)]
pub enum ClickHouseError {
    #[error("ClickHouse connection error: {0}")]
    ConnectionError(String),
    #[error("Query error: {0}")]
    QueryError(String),
    #[error("Insert error: {0}")]
    InsertError(String),
}

#[derive(Debug, Clone, Row, Serialize, Deserialize)]
struct SensorRow {
    device_id: String,
    timestamp: i64,
    cam_angle: f64,
    duitou_acceleration: f64,
    grain_reaction_force: f64,
    frame_vibration_x: f64,
    frame_vibration_y: f64,
    frame_vibration_z: f64,
    frame_vibration_total: f64,
    water_wheel_speed: f64,
    duitou_position: f64,
}

#[derive(Debug, Clone, Row, Serialize, Deserialize)]
struct DynamicsRow {
    device_id: String,
    timestamp: i64,
    cam_angle: f64,
    pounding_force: f64,
    impact_energy: f64,
    duitou_velocity: f64,
    duitou_displacement: f64,
    contact_time: f64,
    restitution_coefficient: f64,
    friction_force: f64,
}

#[derive(Debug, Clone, Row, Serialize, Deserialize)]
struct AlertRow {
    id: String,
    device_id: String,
    timestamp: i64,
    alert_type: String,
    alert_level: String,
    alert_message: String,
    alert_value: f64,
    threshold: f64,
    acknowledged: bool,
}

pub struct ClickHouseClient {
    client: Client,
}

impl ClickHouseClient {
    pub fn new(url: &str, database: &str) -> Result<Self, ClickHouseError> {
        let client = Client::default()
            .with_url(url)
            .with_database(database);

        Ok(ClickHouseClient { client })
    }

    pub async fn insert_sensor_data(&self, data: &[SensorData]) -> Result<(), ClickHouseError> {
        let rows: Vec<SensorRow> = data
            .iter()
            .map(|d| SensorRow {
                device_id: d.device_id.clone(),
                timestamp: d.timestamp.timestamp_millis(),
                cam_angle: d.cam_angle,
                duitou_acceleration: d.duitou_acceleration,
                grain_reaction_force: d.grain_reaction_force,
                frame_vibration_x: d.frame_vibration_x,
                frame_vibration_y: d.frame_vibration_y,
                frame_vibration_z: d.frame_vibration_z,
                frame_vibration_total: d.frame_vibration_total,
                water_wheel_speed: d.water_wheel_speed,
                duitou_position: d.duitou_position,
            })
            .collect();

        let mut insert = self.client.insert("sensor_data").map_err(|e| {
            ClickHouseError::InsertError(e.to_string())
        })?;

        for row in rows {
            insert.write(&row).await.map_err(|e| {
                ClickHouseError::InsertError(e.to_string())
            })?;
        }

        insert.end().await.map_err(|e| {
            ClickHouseError::InsertError(e.to_string())
        })?;

        Ok(())
    }

    pub async fn insert_dynamics_result(
        &self,
        results: &[DynamicsResult],
    ) -> Result<(), ClickHouseError> {
        let rows: Vec<DynamicsRow> = results
            .iter()
            .map(|d| DynamicsRow {
                device_id: d.device_id.clone(),
                timestamp: d.timestamp.timestamp_millis(),
                cam_angle: d.cam_angle,
                pounding_force: d.pounding_force,
                impact_energy: d.impact_energy,
                duitou_velocity: d.duitou_velocity,
                duitou_displacement: d.duitou_displacement,
                contact_time: d.contact_time,
                restitution_coefficient: d.restitution_coefficient,
                friction_force: d.friction_force,
            })
            .collect();

        let mut insert = self.client.insert("dynamics_simulation").map_err(|e| {
            ClickHouseError::InsertError(e.to_string())
        })?;

        for row in rows {
            insert.write(&row).await.map_err(|e| {
                ClickHouseError::InsertError(e.to_string())
            })?;
        }

        insert.end().await.map_err(|e| {
            ClickHouseError::InsertError(e.to_string())
        })?;

        Ok(())
    }

    pub async fn insert_alert(&self, alert: &Alert) -> Result<(), ClickHouseError> {
        let row = AlertRow {
            id: alert.id.clone(),
            device_id: alert.device_id.clone(),
            timestamp: alert.timestamp.timestamp_millis(),
            alert_type: alert.alert_type.clone(),
            alert_level: alert.alert_level.clone(),
            alert_message: alert.alert_message.clone(),
            alert_value: alert.alert_value,
            threshold: alert.threshold,
            acknowledged: false,
        };

        let mut insert = self.client.insert("alerts").map_err(|e| {
            ClickHouseError::InsertError(e.to_string())
        })?;

        insert.write(&row).await.map_err(|e| {
            ClickHouseError::InsertError(e.to_string())
        })?;

        insert.end().await.map_err(|e| {
            ClickHouseError::InsertError(e.to_string())
        })?;

        Ok(())
    }

    pub async fn query_recent_sensor_data(
        &self,
        device_id: &str,
        limit: u64,
    ) -> Result<Vec<SensorData>, ClickHouseError> {
        let query = format!(
            "SELECT device_id, timestamp, cam_angle, duitou_acceleration, grain_reaction_force,
                    frame_vibration_x, frame_vibration_y, frame_vibration_z, frame_vibration_total,
                    water_wheel_speed, duitou_position
             FROM sensor_data
             WHERE device_id = '{}'
             ORDER BY timestamp DESC
             LIMIT {}",
            device_id, limit
        );

        let rows: Vec<SensorRow> = self
            .client
            .query(&query)
            .fetch_all()
            .await
            .map_err(|e| ClickHouseError::QueryError(e.to_string()))?;

        Ok(rows
            .into_iter()
            .map(|r| SensorData {
                device_id: r.device_id,
                timestamp: chrono::DateTime::from_timestamp_millis(r.timestamp)
                    .unwrap_or_else(|| chrono::Utc::now()),
                cam_angle: r.cam_angle,
                duitou_acceleration: r.duitou_acceleration,
                grain_reaction_force: r.grain_reaction_force,
                frame_vibration_x: r.frame_vibration_x,
                frame_vibration_y: r.frame_vibration_y,
                frame_vibration_z: r.frame_vibration_z,
                frame_vibration_total: r.frame_vibration_total,
                water_wheel_speed: r.water_wheel_speed,
                duitou_position: r.duitou_position,
            })
            .collect())
    }

    pub async fn query_recent_dynamics(
        &self,
        device_id: &str,
        limit: u64,
    ) -> Result<Vec<DynamicsResult>, ClickHouseError> {
        let query = format!(
            "SELECT device_id, timestamp, cam_angle, pounding_force, impact_energy,
                    duitou_velocity, duitou_displacement, contact_time,
                    restitution_coefficient, friction_force
             FROM dynamics_simulation
             WHERE device_id = '{}'
             ORDER BY timestamp DESC
             LIMIT {}",
            device_id, limit
        );

        let rows: Vec<DynamicsRow> = self
            .client
            .query(&query)
            .fetch_all()
            .await
            .map_err(|e| ClickHouseError::QueryError(e.to_string()))?;

        Ok(rows
            .into_iter()
            .map(|r| DynamicsResult {
                device_id: r.device_id,
                timestamp: chrono::DateTime::from_timestamp_millis(r.timestamp)
                    .unwrap_or_else(|| chrono::Utc::now()),
                cam_angle: r.cam_angle,
                pounding_force: r.pounding_force,
                impact_energy: r.impact_energy,
                duitou_velocity: r.duitou_velocity,
                duitou_displacement: r.duitou_displacement,
                contact_time: r.contact_time,
                restitution_coefficient: r.restitution_coefficient,
                friction_force: r.friction_force,
            })
            .collect())
    }

    pub async fn query_recent_alerts(
        &self,
        device_id: Option<&str>,
        limit: u64,
    ) -> Result<Vec<Alert>, ClickHouseError> {
        let query = match device_id {
            Some(id) => format!(
                "SELECT id, device_id, timestamp, alert_type, alert_level, alert_message,
                        alert_value, threshold, acknowledged
                 FROM alerts
                 WHERE device_id = '{}'
                 ORDER BY timestamp DESC
                 LIMIT {}",
                id, limit
            ),
            None => format!(
                "SELECT id, device_id, timestamp, alert_type, alert_level, alert_message,
                        alert_value, threshold, acknowledged
                 FROM alerts
                 ORDER BY timestamp DESC
                 LIMIT {}",
                limit
            ),
        };

        let rows: Vec<AlertRow> = self
            .client
            .query(&query)
            .fetch_all()
            .await
            .map_err(|e| ClickHouseError::QueryError(e.to_string()))?;

        Ok(rows
            .into_iter()
            .map(|r| Alert {
                id: r.id,
                device_id: r.device_id,
                timestamp: chrono::DateTime::from_timestamp_millis(r.timestamp)
                    .unwrap_or_else(|| chrono::Utc::now()),
                alert_type: r.alert_type,
                alert_level: r.alert_level,
                alert_message: r.alert_message,
                alert_value: r.alert_value,
                threshold: r.threshold,
            })
            .collect())
    }

    pub async fn get_device_info(
        &self,
        device_id: &str,
    ) -> Result<Option<DeviceInfo>, ClickHouseError> {
        let query = format!(
            "SELECT device_id, device_name, location, cam_base_radius, cam_lift,
                    duitou_mass, water_flow_rate, frame_vibration_threshold
             FROM devices
             WHERE device_id = '{}'
             LIMIT 1",
            device_id
        );

        #[derive(Debug, Row, Deserialize)]
        struct DeviceRow {
            device_id: String,
            device_name: String,
            location: String,
            cam_base_radius: f64,
            cam_lift: f64,
            duitou_mass: f64,
            water_flow_rate: f64,
            frame_vibration_threshold: f64,
        }

        let result: Option<DeviceRow> = self
            .client
            .query(&query)
            .fetch_optional()
            .await
            .map_err(|e| ClickHouseError::QueryError(e.to_string()))?;

        Ok(result.map(|r| DeviceInfo {
            device_id: r.device_id,
            device_name: r.device_name,
            location: r.location,
            cam_base_radius: r.cam_base_radius,
            cam_lift: r.cam_lift,
            duitou_mass: r.duitou_mass,
            water_flow_rate: r.water_flow_rate,
            frame_vibration_threshold: r.frame_vibration_threshold,
        }))
    }

    pub async fn get_all_devices(&self) -> Result<Vec<DeviceInfo>, ClickHouseError> {
        let query = r#"
            SELECT device_id, device_name, location, cam_base_radius, cam_lift,
                   duitou_mass, water_flow_rate, frame_vibration_threshold
            FROM devices
            WHERE is_active = true
            ORDER BY device_id
        "#;

        #[derive(Debug, Row, Deserialize)]
        struct DeviceRow {
            device_id: String,
            device_name: String,
            location: String,
            cam_base_radius: f64,
            cam_lift: f64,
            duitou_mass: f64,
            water_flow_rate: f64,
            frame_vibration_threshold: f64,
        }

        let rows: Vec<DeviceRow> = self
            .client
            .query(query)
            .fetch_all()
            .await
            .map_err(|e| ClickHouseError::QueryError(e.to_string()))?;

        Ok(rows
            .into_iter()
            .map(|r| DeviceInfo {
                device_id: r.device_id,
                device_name: r.device_name,
                location: r.location,
                cam_base_radius: r.cam_base_radius,
                cam_lift: r.cam_lift,
                duitou_mass: r.duitou_mass,
                water_flow_rate: r.water_flow_rate,
                frame_vibration_threshold: r.frame_vibration_threshold,
            })
            .collect())
    }

    pub async fn insert_optimization_result(
        &self,
        result: &OptimizationResult,
    ) -> Result<(), ClickHouseError> {
        #[derive(Debug, Row, Serialize)]
        struct OptRow {
            id: String,
            device_id: String,
            timestamp: i64,
            cam_base_radius: f64,
            cam_lift: f64,
            cam_pressure_angle: f64,
            cam_profile_type: String,
            target_efficiency: f64,
            actual_efficiency: f64,
            average_pounding_force: f64,
            impact_energy_per_cycle: f64,
            husking_rate: f64,
            grain_breakage_rate: f64,
            optimization_parameters: String,
        }

        let params = serde_json::to_string(&result.cam_profile)
            .unwrap_or_else(|_| "[]".to_string());

        let row = OptRow {
            id: result.optimization_id.clone(),
            device_id: result.device_id.clone(),
            timestamp: result.timestamp.timestamp_millis(),
            cam_base_radius: result.base_radius,
            cam_lift: result.lift,
            cam_pressure_angle: 0.0,
            cam_profile_type: result.cam_profile_type.clone(),
            target_efficiency: 0.0,
            actual_efficiency: result.overall_efficiency,
            average_pounding_force: result.pounding_force,
            impact_energy_per_cycle: result.impact_energy,
            husking_rate: result.husking_rate,
            grain_breakage_rate: result.breakage_rate,
            optimization_parameters: params,
        };

        let mut insert = self
            .client
            .insert("optimization_results")
            .map_err(|e| ClickHouseError::InsertError(e.to_string()))?;

        insert
            .write(&row)
            .await
            .map_err(|e| ClickHouseError::InsertError(e.to_string()))?;

        insert
            .end()
            .await
            .map_err(|e| ClickHouseError::InsertError(e.to_string()))?;

        Ok(())
    }

    pub async fn insert_cam_comparison_result(
        &self,
        result: &crate::models::CamProfileComparisonResult,
    ) -> Result<(), ClickHouseError> {

        #[derive(Debug, Row, Serialize)]
        struct ComparisonRow {
            comparison_id: String,
            device_id: String,
            grain_type: String,
            best_profile: String,
            profile_types: Vec<String>,
            profile_names: Vec<String>,
            efficiencies: Vec<f64>,
            husking_rates: Vec<f64>,
            breakage_rates: Vec<f64>,
            pounding_forces: Vec<f64>,
            impact_energies: Vec<f64>,
            max_jerks: Vec<f64>,
            pressure_angles: Vec<f64>,
            min_curvatures: Vec<f64>,
            manufacturing_costs: Vec<f64>,
            scores: Vec<f64>,
            timestamp: i64,
        }

        let row = ComparisonRow {
            comparison_id: result.comparison_id.clone(),
            device_id: result.device_id.clone(),
            grain_type: result.grain_type.clone(),
            best_profile: result.best_profile.clone(),
            profile_types: result.results.iter().map(|r| r.profile_type.clone()).collect(),
            profile_names: result.results.iter().map(|r| r.profile_name_cn.clone()).collect(),
            efficiencies: result.results.iter().map(|r| r.overall_efficiency).collect(),
            husking_rates: result.results.iter().map(|r| r.husking_rate).collect(),
            breakage_rates: result.results.iter().map(|r| r.breakage_rate).collect(),
            pounding_forces: result.results.iter().map(|r| r.pounding_force).collect(),
            impact_energies: result.results.iter().map(|r| r.impact_energy).collect(),
            max_jerks: result.results.iter().map(|r| r.max_jerk).collect(),
            pressure_angles: result.results.iter().map(|r| r.max_pressure_angle).collect(),
            min_curvatures: result.results.iter().map(|r| r.min_curvature).collect(),
            manufacturing_costs: result.results.iter().map(|r| r.manufacturing_cost).collect(),
            scores: result.results.iter().map(|r| r.score).collect(),
            timestamp: result.timestamp.timestamp_millis(),
        };

        let mut insert = self.client.insert("cam_profile_comparison_results")
            .map_err(|e| ClickHouseError::InsertError(e.to_string()))?;
        insert.write(&row).await.map_err(|e| ClickHouseError::InsertError(e.to_string()))?;
        insert.end().await.map_err(|e| ClickHouseError::InsertError(e.to_string()))?;

        Ok(())
    }

    pub async fn insert_cross_era_result(
        &self,
        result: &crate::models::CrossEraComparisonResult,
    ) -> Result<(), ClickHouseError> {

        #[derive(Debug, Row, Serialize)]
        struct CrossEraRow {
            comparison_id: String,
            grain_type: String,
            ancient_name: String,
            ancient_power_kw: f64,
            ancient_efficiency: f64,
            ancient_pounding_rate_kg_h: f64,
            ancient_energy_kwh_100kg: f64,
            ancient_husking_rate: f64,
            ancient_breakage_rate: f64,
            ancient_noise_db: f64,
            ancient_cost_cny: f64,
            ancient_lifespan_years: f64,
            modern_name: String,
            modern_power_kw: f64,
            modern_efficiency: f64,
            modern_pounding_rate_kg_h: f64,
            modern_energy_kwh_100kg: f64,
            modern_husking_rate: f64,
            modern_breakage_rate: f64,
            modern_noise_db: f64,
            modern_cost_cny: f64,
            modern_lifespan_years: f64,
            efficiency_ratio: f64,
            productivity_ratio: f64,
            energy_ratio: f64,
            timestamp: i64,
        }

        let row = CrossEraRow {
            comparison_id: result.comparison_id.clone(),
            grain_type: result.grain_type.clone(),
            ancient_name: result.ancient.name.clone(),
            ancient_power_kw: result.ancient.power_kw,
            ancient_efficiency: result.ancient.efficiency,
            ancient_pounding_rate_kg_h: result.ancient.pounding_rate_kg_h,
            ancient_energy_kwh_100kg: result.ancient.energy_consumption_kwh_100kg,
            ancient_husking_rate: result.ancient.husking_rate,
            ancient_breakage_rate: result.ancient.breakage_rate,
            ancient_noise_db: result.ancient.noise_db,
            ancient_cost_cny: result.ancient.cost_cny,
            ancient_lifespan_years: result.ancient.lifespan_years,
            modern_name: result.modern.name.clone(),
            modern_power_kw: result.modern.power_kw,
            modern_efficiency: result.modern.efficiency,
            modern_pounding_rate_kg_h: result.modern.pounding_rate_kg_h,
            modern_energy_kwh_100kg: result.modern.energy_consumption_kwh_100kg,
            modern_husking_rate: result.modern.husking_rate,
            modern_breakage_rate: result.modern.breakage_rate,
            modern_noise_db: result.modern.noise_db,
            modern_cost_cny: result.modern.cost_cny,
            modern_lifespan_years: result.modern.lifespan_years,
            efficiency_ratio: result.efficiency_ratio,
            productivity_ratio: result.productivity_ratio,
            energy_ratio: result.energy_ratio,
            timestamp: result.timestamp.timestamp_millis(),
        };

        let mut insert = self.client.insert("cross_era_comparison_results")
            .map_err(|e| ClickHouseError::InsertError(e.to_string()))?;
        insert.write(&row).await.map_err(|e| ClickHouseError::InsertError(e.to_string()))?;
        insert.end().await.map_err(|e| ClickHouseError::InsertError(e.to_string()))?;

        Ok(())
    }

    pub async fn insert_vibration_interference_result(
        &self,
        result: &crate::models::VibrationInterferenceResult,
    ) -> Result<(), ClickHouseError> {

        #[derive(Debug, Row, Serialize)]
        struct VibrationRow {
            analysis_id: String,
            device_ids: Vec<String>,
            device_phases: Vec<f64>,
            device_positions_x: Vec<f64>,
            device_positions_y: Vec<f64>,
            device_frequencies: Vec<f64>,
            device_amplitudes: Vec<f64>,
            max_interference: f64,
            avg_interference: f64,
            resonance_count: u32,
            safety_level: String,
            recommendation: String,
            timestamp: i64,
        }

        let row = VibrationRow {
            analysis_id: result.analysis_id.clone(),
            device_ids: result.device_states.iter().map(|s| s.device_id.clone()).collect(),
            device_phases: result.device_states.iter().map(|s| s.phase_offset).collect(),
            device_positions_x: result.device_states.iter().map(|s| s.position.0).collect(),
            device_positions_y: result.device_states.iter().map(|s| s.position.1).collect(),
            device_frequencies: result.device_states.iter().map(|s| s.frequency).collect(),
            device_amplitudes: result.device_states.iter().map(|s| s.amplitude).collect(),
            max_interference: result.max_interference,
            avg_interference: result.avg_interference,
            resonance_count: result.resonance_count,
            safety_level: result.safety_level.clone(),
            recommendation: result.recommendation.clone(),
            timestamp: result.timestamp.timestamp_millis(),
        };

        let mut insert = self.client.insert("vibration_interference_results")
            .map_err(|e| ClickHouseError::InsertError(e.to_string()))?;
        insert.write(&row).await.map_err(|e| ClickHouseError::InsertError(e.to_string()))?;
        insert.end().await.map_err(|e| ClickHouseError::InsertError(e.to_string()))?;

        Ok(())
    }

    pub async fn insert_user_cam_design_result(
        &self,
        result: &crate::models::UserCamDesignResult,
    ) -> Result<(), ClickHouseError> {

        #[derive(Debug, Row, Serialize)]
        struct UserDesignRow {
            design_id: String,
            design_name: String,
            user_id: Option<String>,
            grain_type: String,
            base_radius: f64,
            overall_efficiency: f64,
            husking_rate: f64,
            breakage_rate: f64,
            pounding_force: f64,
            impact_energy: f64,
            overall_score: f64,
            grade: String,
            design_feedback: Vec<String>,
            safety_warnings: Vec<String>,
            tolerance_min_curvature: f64,
            tolerance_feasibility: f64,
            tolerance_manufacturing_cost: f64,
            user_defined_lifts: Vec<f64>,
            timestamp: i64,
        }

        let row = UserDesignRow {
            design_id: result.design_id.clone(),
            design_name: result.design_name.clone(),
            user_id: None,
            grain_type: "rice".to_string(),
            base_radius: 0.15,
            overall_efficiency: result.overall_efficiency,
            husking_rate: result.husking_rate,
            breakage_rate: result.breakage_rate,
            pounding_force: result.pounding_force,
            impact_energy: result.impact_energy,
            overall_score: result.overall_score,
            grade: result.grade.clone(),
            design_feedback: result.design_feedback.clone(),
            safety_warnings: result.safety_warnings.clone(),
            tolerance_min_curvature: result.tolerance_report.min_curvature,
            tolerance_feasibility: result.tolerance_report.overall_feasibility,
            tolerance_manufacturing_cost: result.tolerance_report.manufacturing_cost,
            user_defined_lifts: result.cam_profile.iter().map(|p| p.lift).collect(),
            timestamp: result.timestamp.timestamp_millis(),
        };

        let mut insert = self.client.insert("user_cam_design_results")
            .map_err(|e| ClickHouseError::InsertError(e.to_string()))?;
        insert.write(&row).await.map_err(|e| ClickHouseError::InsertError(e.to_string()))?;
        insert.end().await.map_err(|e| ClickHouseError::InsertError(e.to_string()))?;

        Ok(())
    }
}
