-- 古代水碓凸轮机构动力学仿真数据库初始化脚本
-- ClickHouse 23.x+

CREATE DATABASE IF NOT EXISTS shuidui
ENGINE = Atomic;

USE shuidui;

-- 传感器原始数据表：每分钟上报一次
CREATE TABLE IF NOT EXISTS sensor_data (
    device_id String,
    timestamp DateTime64(3, 'Asia/Shanghai'),
    cam_angle Float64,
    duitou_acceleration Float64,
    grain_reaction_force Float64,
    frame_vibration_x Float64,
    frame_vibration_y Float64,
    frame_vibration_z Float64,
    frame_vibration_total Float64,
    water_wheel_speed Float64,
    duitou_position Float64
)
ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (device_id, timestamp)
TTL timestamp + INTERVAL 1 YEAR
COMMENT '水碓传感器原始数据表';

-- 动力学仿真结果表
CREATE TABLE IF NOT EXISTS dynamics_simulation (
    device_id String,
    timestamp DateTime64(3, 'Asia/Shanghai'),
    cam_angle Float64,
    pounding_force Float64,
    impact_energy Float64,
    duitou_velocity Float64,
    duitou_displacement Float64,
    contact_time Float64,
    restitution_coefficient Float64,
    friction_force Float64
)
ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (device_id, timestamp)
TTL timestamp + INTERVAL 1 YEAR
COMMENT '凸轮动力学仿真结果表';

-- 告警记录表
CREATE TABLE IF NOT EXISTS alerts (
    id UUID,
    device_id String,
    timestamp DateTime64(3, 'Asia/Shanghai'),
    alert_type String,
    alert_level String,
    alert_message String,
    alert_value Float64,
    threshold Float64,
    acknowledged Bool DEFAULT false
)
ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (device_id, timestamp, alert_type)
TTL timestamp + INTERVAL 1 YEAR
COMMENT '告警记录表';

-- 舂捣力优化结果表
CREATE TABLE IF NOT EXISTS optimization_results (
    id UUID,
    device_id String,
    timestamp DateTime64(3, 'Asia/Shanghai'),
    cam_base_radius Float64,
    cam_lift Float64,
    cam_pressure_angle Float64,
    cam_profile_type String,
    target_efficiency Float64,
    actual_efficiency Float64,
    average_pounding_force Float64,
    impact_energy_per_cycle Float64,
    husking_rate Float64,
    grain_breakage_rate Float64,
    optimization_parameters String
)
ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (device_id, timestamp)
TTL timestamp + INTERVAL 2 YEAR
COMMENT '舂捣力优化结果表';

-- 设备元数据表
CREATE TABLE IF NOT EXISTS devices (
    device_id String,
    device_name String,
    location String,
    installation_date Date,
    cam_base_radius Float64,
    cam_lift Float64,
    duitou_mass Float64,
    water_flow_rate Float64,
    frame_vibration_threshold Float64,
    is_active Bool DEFAULT true
)
ENGINE = ReplacingMergeTree()
ORDER BY device_id
COMMENT '水碓设备元数据表';

-- 插入默认设备数据
INSERT INTO devices (device_id, device_name, location, installation_date, cam_base_radius, cam_lift, duitou_mass, water_flow_rate, frame_vibration_threshold, is_active) VALUES
('shuidui-001', '汉代一号水碓', '河南南阳考古现场', '2024-03-15', 0.15, 0.12, 25.0, 0.05, 5.0, true),
('shuidui-002', '汉代二号水碓', '陕西西安考古现场', '2024-06-20', 0.18, 0.15, 32.0, 0.06, 5.0, true),
('shuidui-003', '汉代三号水碓', '四川成都考古现场', '2024-09-10', 0.12, 0.10, 20.0, 0.04, 5.0, true);

-- ============ 新功能表：凸轮效率对比结果 ============
CREATE TABLE IF NOT EXISTS cam_profile_comparison_results (
    comparison_id String,
    device_id String,
    grain_type String,
    best_profile String,
    profile_types Array(String),
    profile_names Array(String),
    efficiencies Array(Float64),
    husking_rates Array(Float64),
    breakage_rates Array(Float64),
    pounding_forces Array(Float64),
    impact_energies Array(Float64),
    max_jerks Array(Float64),
    pressure_angles Array(Float64),
    min_curvatures Array(Float64),
    manufacturing_costs Array(Float64),
    scores Array(Float64),
    timestamp DateTime64(9, 'UTC') DEFAULT now64(9)
) ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (device_id, timestamp)
TTL timestamp + INTERVAL 2 YEAR
COMMENT '凸轮形状效率对比结果表';

-- ============ 新功能表：跨时代效率对比结果 ============
CREATE TABLE IF NOT EXISTS cross_era_comparison_results (
    comparison_id String,
    grain_type String,
    ancient_name String,
    ancient_power_kw Float64,
    ancient_efficiency Float64,
    ancient_pounding_rate_kg_h Float64,
    ancient_energy_kwh_100kg Float64,
    ancient_husking_rate Float64,
    ancient_breakage_rate Float64,
    ancient_noise_db Float64,
    ancient_cost_cny Float64,
    ancient_lifespan_years Float64,
    modern_name String,
    modern_power_kw Float64,
    modern_efficiency Float64,
    modern_pounding_rate_kg_h Float64,
    modern_energy_kwh_100kg Float64,
    modern_husking_rate Float64,
    modern_breakage_rate Float64,
    modern_noise_db Float64,
    modern_cost_cny Float64,
    modern_lifespan_years Float64,
    efficiency_ratio Float64,
    productivity_ratio Float64,
    energy_ratio Float64,
    timestamp DateTime64(9, 'UTC') DEFAULT now64(9)
) ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (timestamp)
TTL timestamp + INTERVAL 3 YEAR
COMMENT '古今跨时代效率对比结果表';

-- ============ 新功能表：振动干涉分析结果 ============
CREATE TABLE IF NOT EXISTS vibration_interference_results (
    analysis_id String,
    device_ids Array(String),
    device_phases Array(Float64),
    device_positions_x Array(Float64),
    device_positions_y Array(Float64),
    device_frequencies Array(Float64),
    device_amplitudes Array(Float64),
    max_interference Float64,
    avg_interference Float64,
    resonance_count UInt32,
    safety_level String,
    recommendation String,
    timestamp DateTime64(9, 'UTC') DEFAULT now64(9)
) ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (timestamp)
TTL timestamp + INTERVAL 1 YEAR
COMMENT '多台水碓振动干涉分析结果表';

-- ============ 新功能表：用户凸轮设计体验结果 ============
CREATE TABLE IF NOT EXISTS user_cam_design_results (
    design_id String,
    design_name String,
    user_id Nullable(String),
    grain_type String,
    base_radius Float64,
    overall_efficiency Float64,
    husking_rate Float64,
    breakage_rate Float64,
    pounding_force Float64,
    impact_energy Float64,
    overall_score Float64,
    grade String,
    design_feedback Array(String),
    safety_warnings Array(String),
    tolerance_min_curvature Float64,
    tolerance_feasibility Float64,
    tolerance_manufacturing_cost Float64,
    user_defined_lifts Array(Float64),
    timestamp DateTime64(9, 'UTC') DEFAULT now64(9)
) ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (timestamp)
TTL timestamp + INTERVAL 5 YEAR
COMMENT '公众虚拟凸轮设计体验结果表';

-- ============ 新功能表：现代电动舂米机规格 ============
CREATE TABLE IF NOT EXISTS modern_rice_mill_specs (
    spec_id String,
    model_name String,
    power_kw Float64,
    motor_rpm Float64,
    transmission_ratio Float64,
    mechanical_efficiency Float64,
    typical_husking_rate Float64,
    typical_breakage_rate Float64,
    price_cny Float64,
    manufacturer String,
    create_time DateTime64(9, 'UTC') DEFAULT now64(9)
) ENGINE = ReplacingMergeTree(create_time)
ORDER BY spec_id
TTL create_time + INTERVAL 10 YEAR
COMMENT '现代电动舂米机规格参数表';

INSERT INTO modern_rice_mill_specs (
    spec_id, model_name, power_kw, motor_rpm, transmission_ratio,
    mechanical_efficiency, typical_husking_rate, typical_breakage_rate,
    price_cny, manufacturer
) VALUES
('mill-001', '家庭小型电动舂米机', 1.5, 1450, 30.0, 0.82, 0.90, 0.04, 1500.0, '国产农机'),
('mill-002', '中型商用电动舂米机', 3.0, 1450, 25.0, 0.85, 0.93, 0.03, 3500.0, '国产农机'),
('mill-003', '大型工业电动舂米机', 7.5, 960, 20.0, 0.88, 0.95, 0.025, 8000.0, '合资品牌');

-- 创建物化视图：每分钟统计
CREATE MATERIALIZED VIEW IF NOT EXISTS sensor_stats_1min
ENGINE = SummingMergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (device_id, timestamp)
AS
SELECT
    device_id,
    toStartOfMinute(timestamp) AS timestamp,
    count() AS sample_count,
    avg(cam_angle) AS avg_cam_angle,
    max(cam_angle) AS max_cam_angle,
    avg(duitou_acceleration) AS avg_acceleration,
    max(abs(duitou_acceleration)) AS max_acceleration,
    avg(grain_reaction_force) AS avg_grain_force,
    max(grain_reaction_force) AS max_grain_force,
    avg(frame_vibration_total) AS avg_vibration,
    max(frame_vibration_total) AS max_vibration
FROM sensor_data
GROUP BY device_id, timestamp;

-- 创建告警触发器视图
CREATE MATERIALIZED VIEW IF NOT EXISTS vibration_alerts_mv
TO alerts
AS
SELECT
    generateUUIDv4() AS id,
    device_id,
    timestamp,
    'frame_vibration' AS alert_type,
    CASE
        WHEN frame_vibration_total > 8.0 THEN 'critical'
        WHEN frame_vibration_total > 5.0 THEN 'warning'
        ELSE 'info'
    END AS alert_level,
    CASE
        WHEN frame_vibration_total > 8.0 THEN '机架振动严重超限，请立即停机检查！'
        WHEN frame_vibration_total > 5.0 THEN '机架振动超过预警阈值'
        ELSE '振动正常'
    END AS alert_message,
    frame_vibration_total AS alert_value,
    5.0 AS threshold,
    false AS acknowledged
FROM sensor_data
WHERE frame_vibration_total > 5.0;
