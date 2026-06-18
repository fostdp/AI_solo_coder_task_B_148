use crate::dynamics::{PenaltyContactModel, CamDynamicsSimulator};
use crate::config::DynamicsConfig;
use crate::models::{DeviceInfo, SensorData, DynamicsResult};

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{debug, warn};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct DynamicsTask {
    pub task_id: String,
    pub device: DeviceInfo,
    pub sensor: SensorData,
    pub contact_model: PenaltyContactModel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicsTaskResult {
    pub task_id: String,
    pub device_id: String,
    pub result: DynamicsResult,
    pub compute_time_ms: u64,
}

pub struct DynamicsPool {
    config: DynamicsConfig,
    num_threads: usize,
    sender: mpsc::UnboundedSender<DynamicsTask>,
    handles: Vec<JoinHandle<()>>,
}

impl DynamicsPool {
    pub fn new(num_threads: usize, config: DynamicsConfig) -> (Self, mpsc::UnboundedReceiver<DynamicsTaskResult>) {
        let (task_tx, task_rx) = mpsc::unbounded_channel::<DynamicsTask>();
        let (result_tx, result_rx) = mpsc::unbounded_channel::<DynamicsTaskResult>();

        let config_arc = Arc::new(config.clone());
        let task_rx_arc = Arc::new(tokio::sync::Mutex::new(task_rx));
        let mut handles = Vec::with_capacity(num_threads);

        for i in 0..num_threads {
            let result_tx_clone = result_tx.clone();
            let config_clone = config_arc.clone();
            let rx = task_rx_arc.clone();

            let handle = tokio::spawn(async move {
                debug!("Dynamics worker {} started", i);
                loop {
                    let task = {
                        let mut guard = rx.lock().await;
                        guard.recv().await
                    };
                    match task {
                        Some(t) => {
                            let start = std::time::Instant::now();
                            let result = Self::process_task(&t, &config_clone);
                            let elapsed = start.elapsed();

                            let task_result = DynamicsTaskResult {
                                task_id: t.task_id,
                                device_id: t.device.device_id,
                                result,
                                compute_time_ms: elapsed.as_millis() as u64,
                            };

                            if result_tx_clone.send(task_result).is_err() {
                                warn!("Dynamics worker {} failed to send result", i);
                                break;
                            }
                        }
                        None => break,
                    }
                }
                debug!("Dynamics worker {} stopped", i);
            });
            handles.push(handle);
        }

        let pool = DynamicsPool {
            config,
            num_threads,
            sender: task_tx,
            handles,
        };

        (pool, result_rx)
    }

    pub fn submit(&self, task: DynamicsTask) -> Result<(), String> {
        self.sender
            .send(task)
            .map_err(|e| format!("Failed to submit dynamics task: {}", e))
    }

    pub fn submit_simple(
        &self,
        task_id: String,
        device: DeviceInfo,
        sensor: SensorData,
    ) -> Result<(), String> {
        let contact_model = PenaltyContactModel::new(
            device.cam_base_radius,
            &self.config,
        );
        let task = DynamicsTask {
            task_id,
            device,
            sensor,
            contact_model,
        };
        self.submit(task)
    }

    fn process_task(task: &DynamicsTask, config: &DynamicsConfig) -> DynamicsResult {
        let mut simulator = CamDynamicsSimulator::new_with_config(
            task.device.clone(),
            config,
        );
        simulator.simulate(&task.sensor)
    }

    pub fn num_threads(&self) -> usize {
        self.num_threads
    }

    pub fn shutdown(&mut self) {
        for handle in self.handles.drain(..) {
            handle.abort();
        }
    }
}

impl Drop for DynamicsPool {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::optimization::create_test_device;
    use chrono::Utc;

    fn make_test_sensor(device_id: &str) -> SensorData {
        SensorData {
            device_id: device_id.to_string(),
            timestamp: Utc::now(),
            cam_angle: 90.0,
            duitou_acceleration: 0.0,
            grain_reaction_force: 0.0,
            frame_vibration_x: 0.0,
            frame_vibration_y: 0.0,
            frame_vibration_z: 0.0,
            frame_vibration_total: 0.0,
            water_wheel_speed: 3.14,
            duitou_position: 0.06,
        }
    }

    #[test]
    fn test_process_task_sync() {
        let config = DynamicsConfig::default();
        let device = create_test_device();
        let sensor = make_test_sensor(&device.device_id);
        let task = DynamicsTask {
            task_id: "sync-test".to_string(),
            device: device.clone(),
            sensor,
            contact_model: PenaltyContactModel::new(device.cam_base_radius, &config),
        };

        let result = DynamicsPool::process_task(&task, &config);
        assert!(result.pounding_force >= 0.0);
        assert!(result.impact_energy >= 0.0);
        assert!(result.duitou_velocity >= 0.0 || result.duitou_velocity <= 0.0);
        assert!(result.contact_time >= 0.0);
    }

    #[test]
    fn test_dynamics_task_creation() {
        let config = DynamicsConfig::default();
        let device = create_test_device();
        let sensor = make_test_sensor(&device.device_id);
        let contact = PenaltyContactModel::new(device.cam_base_radius, &config);

        let task = DynamicsTask {
            task_id: "t1".to_string(),
            device: device.clone(),
            sensor: sensor.clone(),
            contact_model: contact,
        };

        assert_eq!(task.task_id, "t1");
        assert_eq!(task.device.device_id, device.device_id);
    }
}
