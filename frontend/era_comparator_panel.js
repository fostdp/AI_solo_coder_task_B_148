(function (global) {
    'use strict';

    let apiBaseUrl = global._camPanelApiBaseUrl || 'http://localhost:8080/api';
    let currentDeviceId = 'shuidui-001';

    function setApiBase(url) { apiBaseUrl = url; }
    function setCurrentDevice(id) { currentDeviceId = id; }

    async function handleCrossEraComparison() {
        if (global.showLoading) global.showLoading('正在进行跨时代效率对比...');

        const motorKw = parseFloat(document.getElementById('crossera-motor').value);

        const request = {
            ancient_device_id: currentDeviceId,
            grain_type: document.getElementById('crossera-grain').value,
            modern_motor_power_kw: motorKw,
            modern_motor_rpm: 1450
        };

        try {
            const response = await fetch(`${apiBaseUrl}/compare/cross-era`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(request)
            });

            const data = await response.json();

            if (data.success && data.data) {
                renderCrossEraResults(data.data);
            } else {
                renderMockCrossEra(motorKw);
            }
        } catch (e) {
            console.error('Cross-era comparison failed:', e);
            renderMockCrossEra(motorKw);
        } finally {
            if (global.hideLoading) global.hideLoading();
        }
    }

    function renderCrossEraResults(result) {
        const container = document.getElementById('crossera-results');
        container.innerHTML = `
            <div class="cross-era-container">
                <div class="era-card ancient">
                    <h3>古代水碓</h3>
                    <div class="era-icon">🏛️</div>
                    <div class="era-metrics">
                        <div class="era-metric-row">
                            <span class="metric-label">名称</span>
                            <span class="metric-value">${result.ancient.name}</span>
                        </div>
                        <div class="era-metric-row">
                            <span class="metric-label">动力源</span>
                            <span class="metric-value">${result.ancient.power_source}</span>
                        </div>
                        <div class="era-metric-row">
                            <span class="metric-label">功率</span>
                            <span class="metric-value">${result.ancient.power_kw.toFixed(2)} kW</span>
                        </div>
                        <div class="era-metric-row">
                            <span class="metric-label">总效率</span>
                            <span class="metric-value">${(result.ancient.efficiency * 100).toFixed(1)}%</span>
                        </div>
                        <div class="era-metric-row">
                            <span class="metric-label">生产率</span>
                            <span class="metric-value">${result.ancient.pounding_rate_kg_h.toFixed(1)} kg/h</span>
                        </div>
                        <div class="era-metric-row">
                            <span class="metric-label">能耗</span>
                            <span class="metric-value">${result.ancient.energy_consumption_kwh_100kg.toFixed(2)} kWh/100kg</span>
                        </div>
                        <div class="era-metric-row">
                            <span class="metric-label">脱壳率</span>
                            <span class="metric-value">${(result.ancient.husking_rate * 100).toFixed(1)}%</span>
                        </div>
                        <div class="era-metric-row">
                            <span class="metric-label">噪音</span>
                            <span class="metric-value">${result.ancient.noise_db.toFixed(0)} dB</span>
                        </div>
                        <div class="era-metric-row">
                            <span class="metric-label">成本</span>
                            <span class="metric-value">¥${result.ancient.cost_cny.toFixed(0)}</span>
                        </div>
                        <div class="era-metric-row">
                            <span class="metric-label">寿命</span>
                            <span class="metric-value">${result.ancient.lifespan_years.toFixed(0)} 年</span>
                        </div>
                    </div>
                </div>
                <div class="vs-divider">VS</div>
                <div class="era-card modern">
                    <h3>现代舂米机</h3>
                    <div class="era-icon">⚡</div>
                    <div class="era-metrics">
                        <div class="era-metric-row">
                            <span class="metric-label">名称</span>
                            <span class="metric-value">${result.modern.name}</span>
                        </div>
                        <div class="era-metric-row">
                            <span class="metric-label">动力源</span>
                            <span class="metric-value">${result.modern.power_source}</span>
                        </div>
                        <div class="era-metric-row">
                            <span class="metric-label">功率</span>
                            <span class="metric-value">${result.modern.power_kw.toFixed(2)} kW</span>
                        </div>
                        <div class="era-metric-row">
                            <span class="metric-label">总效率</span>
                            <span class="metric-value">${(result.modern.efficiency * 100).toFixed(1)}%</span>
                        </div>
                        <div class="era-metric-row">
                            <span class="metric-label">生产率</span>
                            <span class="metric-value">${result.modern.pounding_rate_kg_h.toFixed(1)} kg/h</span>
                        </div>
                        <div class="era-metric-row">
                            <span class="metric-label">能耗</span>
                            <span class="metric-value">${result.modern.energy_consumption_kwh_100kg.toFixed(2)} kWh/100kg</span>
                        </div>
                        <div class="era-metric-row">
                            <span class="metric-label">脱壳率</span>
                            <span class="metric-value">${(result.modern.husking_rate * 100).toFixed(1)}%</span>
                        </div>
                        <div class="era-metric-row">
                            <span class="metric-label">噪音</span>
                            <span class="metric-value">${result.modern.noise_db.toFixed(0)} dB</span>
                        </div>
                        <div class="era-metric-row">
                            <span class="metric-label">成本</span>
                            <span class="metric-value">¥${result.modern.cost_cny.toFixed(0)}</span>
                        </div>
                        <div class="era-metric-row">
                            <span class="metric-label">寿命</span>
                            <span class="metric-value">${result.modern.lifespan_years.toFixed(0)} 年</span>
                        </div>
                    </div>
                </div>
            </div>
            <div class="comparison-summary">
                <h4>📊 对比总结</h4>
                <div class="summary-item">
                    <span class="label">效率提升倍数</span>
                    <span class="value">${result.efficiency_ratio.toFixed(1)}×</span>
                </div>
                <div class="summary-item">
                    <span class="label">生产率提升倍数</span>
                    <span class="value">${result.productivity_ratio.toFixed(1)}×</span>
                </div>
                <div class="summary-item">
                    <span class="label">能耗降低倍数</span>
                    <span class="value">${result.energy_ratio.toFixed(1)}×</span>
                </div>
            </div>
        `;
    }

    function renderMockCrossEra(motorKw) {
        const ancient = {
            name: '汉代水碓', power_source: '水力', power_kw: 0.98, efficiency: 0.65,
            pounding_rate_kg_h: 45, energy_consumption_kwh_100kg: 2.2,
            husking_rate: 0.75, breakage_rate: 0.12,
            noise_db: 75, cost_cny: 5000, lifespan_years: 30
        };

        const modernPower = parseFloat(motorKw);
        const modern = {
            name: `现代电动舂米机 (${modernPower}kW)`,
            power_source: '电力', power_kw: modernPower, efficiency: 0.78,
            pounding_rate_kg_h: modernPower * 80, energy_consumption_kwh_100kg: 0.8,
            husking_rate: 0.92, breakage_rate: 0.03,
            noise_db: 85, cost_cny: 3000, lifespan_years: 10
        };

        renderCrossEraResults({
            ancient, modern,
            efficiency_ratio: modern.efficiency / ancient.efficiency,
            productivity_ratio: modern.pounding_rate_kg_h / ancient.pounding_rate_kg_h,
            energy_ratio: ancient.energy_consumption_kwh_100kg / modern.energy_consumption_kwh_100kg
        });
    }

    function init() {
        const btn = document.getElementById('btn-cross-era');
        if (btn) btn.addEventListener('click', handleCrossEraComparison);
    }

    if (typeof document !== 'undefined') {
        if (document.readyState === 'loading') {
            document.addEventListener('DOMContentLoaded', init);
        } else {
            setTimeout(init, 100);
        }
    }

    global.EraComparatorPanel = {
        handleCrossEraComparison,
        renderCrossEraResults,
        setApiBase,
        setCurrentDevice,
        init
    };

})(typeof window !== 'undefined' ? window : this);
