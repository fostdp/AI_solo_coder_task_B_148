use crate::message_bus::{OptimizerCmdRx, OptimizerCommand};
use crate::config::OptimizationConfig;
use crate::models::{
    DeviceInfo, OptimizationResult, OptimizationRequest,
    CamProfileComparisonRequest, CamProfileComparisonResult,
    CrossEraComparisonRequest, CrossEraComparisonResult,
    VibrationInterferenceRequest, VibrationInterferenceResult,
    UserCamDesignRequest, UserCamDesignResult,
};
use crate::optimization::{PoundingOptimizer, ToleranceAnalysis, VibrationInterferenceAnalyzer};
use crate::config::DynamicsConfig;

use tracing::{info, error};

pub struct ForceOptimizerService {
    config: OptimizationConfig,
    dynamics_config: DynamicsConfig,
    cmd_rx: OptimizerCmdRx,
}

impl ForceOptimizerService {
    pub fn new(
        cmd_rx: OptimizerCmdRx,
        config: OptimizationConfig,
        dynamics_config: DynamicsConfig,
    ) -> Self {
        ForceOptimizerService {
            config,
            dynamics_config,
            cmd_rx,
        }
    }

    pub async fn run(mut self) {
        info!("ForceOptimizerService started");
        while let Some(cmd) = self.cmd_rx.recv().await {
            match cmd {
                OptimizerCommand::Optimize { request, device, reply } => {
                    let start = std::time::Instant::now();
                    let result = self.handle_optimize(request, device);
                    crate::metrics::OPTIMIZATION_DURATION.observe(start.elapsed().as_secs_f64());
                    crate::metrics::OPTIMIZATIONS_RUN.inc();
                    let _ = reply.send(result);
                }
                OptimizerCommand::CompareProfiles { request, device, reply } => {
                    let start = std::time::Instant::now();
                    let result = self.handle_compare_profiles(request, device);
                    crate::metrics::OPTIMIZATION_DURATION.observe(start.elapsed().as_secs_f64());
                    crate::metrics::OPTIMIZATIONS_RUN.inc();
                    let _ = reply.send(result);
                }
                OptimizerCommand::CompareCrossEra { request, device, reply } => {
                    let start = std::time::Instant::now();
                    let result = self.handle_compare_cross_era(request, device);
                    crate::metrics::OPTIMIZATION_DURATION.observe(start.elapsed().as_secs_f64());
                    crate::metrics::OPTIMIZATIONS_RUN.inc();
                    let _ = reply.send(result);
                }
                OptimizerCommand::AnalyzeVibration { request, devices, reply } => {
                    let start = std::time::Instant::now();
                    let result = self.handle_analyze_vibration(request, devices);
                    crate::metrics::OPTIMIZATION_DURATION.observe(start.elapsed().as_secs_f64());
                    crate::metrics::OPTIMIZATIONS_RUN.inc();
                    let _ = reply.send(result);
                }
                OptimizerCommand::TestUserCam { request, device, reply } => {
                    let start = std::time::Instant::now();
                    let result = self.handle_test_user_cam(request, device);
                    crate::metrics::OPTIMIZATION_DURATION.observe(start.elapsed().as_secs_f64());
                    crate::metrics::OPTIMIZATIONS_RUN.inc();
                    let _ = reply.send(result);
                }
            }
        }
        error!("ForceOptimizerService stopped (cmd channel closed)");
    }

    fn handle_optimize(&self, request: OptimizationRequest, device: DeviceInfo) -> OptimizationResult {
        let tolerance = ToleranceAnalysis::from_config(&self.config.tolerance);
        let optimizer = PoundingOptimizer::with_config(
            device,
            tolerance,
            self.dynamics_config.clone(),
        );
        optimizer.optimize(&request)
    }

    fn handle_compare_profiles(
        &self,
        request: CamProfileComparisonRequest,
        device: DeviceInfo,
    ) -> CamProfileComparisonResult {
        let tolerance = ToleranceAnalysis::from_config(&self.config.tolerance);
        let optimizer = PoundingOptimizer::with_config(
            device,
            tolerance,
            self.dynamics_config.clone(),
        );
        optimizer.compare_cam_profiles(&request)
    }

    fn handle_compare_cross_era(
        &self,
        request: CrossEraComparisonRequest,
        device: DeviceInfo,
    ) -> CrossEraComparisonResult {
        let tolerance = ToleranceAnalysis::from_config(&self.config.tolerance);
        let optimizer = PoundingOptimizer::with_config(
            device,
            tolerance,
            self.dynamics_config.clone(),
        );
        optimizer.compare_cross_era(&request)
    }

    fn handle_analyze_vibration(
        &self,
        request: VibrationInterferenceRequest,
        devices: Vec<(DeviceInfo, f64)>,
    ) -> VibrationInterferenceResult {
        let analyzer = VibrationInterferenceAnalyzer::new(devices);
        analyzer.analyze(request.simulation_duration_secs, request.time_step_secs)
    }

    fn handle_test_user_cam(
        &self,
        request: UserCamDesignRequest,
        device: DeviceInfo,
    ) -> UserCamDesignResult {
        let tolerance = ToleranceAnalysis::from_config(&self.config.tolerance);
        let optimizer = PoundingOptimizer::with_config(
            device,
            tolerance,
            self.dynamics_config.clone(),
        );
        optimizer.test_user_cam_design(&request)
    }
}
