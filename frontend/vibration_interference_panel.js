(function (global) {
    'use strict';

    let apiBaseUrl = global._camPanelApiBaseUrl || 'http://localhost:8080/api';

    function setApiBase(url) { apiBaseUrl = url; }

    async function handleVibrationAnalysis() {
        const selectedDevices = Array.from(document.querySelectorAll('#feature-vibration input[type="checkbox"]:checked'))
            .map(cb => cb.value);

        if (selectedDevices.length < 2) {
            alert('请至少选择2台或更多水碓进行干涉分析！');
            return;
        }

        if (global.showLoading) global.showLoading('正在进行多台水碓振动干涉分析...');

        const request = {
            device_ids: selectedDevices,
            simulation_duration_secs: parseFloat(document.getElementById('vib-duration').value),
            time_step_secs: parseFloat(document.getElementById('vib-timestep').value)
        };

        try {
            const response = await fetch(`${apiBaseUrl}/vibration/interference`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(request)
            });

            const data = await response.json();

            if (data.success && data.data) {
                renderVibrationResults(data.data);
            } else {
                renderMockVibration(selectedDevices.length);
            }
        } catch (e) {
            console.error('Vibration analysis failed:', e);
            renderMockVibration(selectedDevices.length);
        } finally {
            if (global.hideLoading) global.hideLoading();
        }
    }

    function renderVibrationResults(result) {
        const container = document.getElementById('vibration-results');

        let safetyClass = '';
        if (result.safety_level === '安全') safetyClass = 'safety-safe';
        else if (result.safety_level === '注意') safetyClass = 'safety-caution';
        else if (result.safety_level === '警告') safetyClass = 'safety-warning';
        else safetyClass = 'safety-danger';

        container.innerHTML = `
            <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:15px;">
                <h3 style="margin:0;">振动干涉分析结果</h3>
                <span class="safety-indicator ${safetyClass}">
                    <span style="width:10px;height:10px;border-radius:50%;background:currentColor;"></span>
                    ${result.safety_level}
                </span>
            </div>

            <div class="stats-grid">
                <div class="stat-item">
                    <div class="stat-label">最大干涉系数</div>
                    <div class="stat-value">${result.max_interference.toFixed(2)}</div>
                </div>
                <div class="stat-item">
                    <div class="stat-label">平均干涉系数</div>
                    <div class="stat-value">${result.avg_interference.toFixed(2)}</div>
                </div>
                <div class="stat-item">
                    <div class="stat-label">共振次数</div>
                    <div class="stat-value">${result.resonance_count}</div>
                </div>
                <div class="stat-item">
                    <div class="stat-label">参与设备</div>
                    <div class="stat-value">${result.device_states.length}台</div>
                </div>
                ${result.max_foundation_vibration !== undefined ? `
                <div class="stat-item">
                    <div class="stat-label">最大地基振动</div>
                    <div class="stat-value">${result.max_foundation_vibration.toFixed(3)}</div>
                </div>
                <div class="stat-item">
                    <div class="stat-label">地基共振次数</div>
                    <div class="stat-value">${result.foundation_resonance_count || 0}</div>
                </div>` : ''}
            </div>

            <canvas class="vibration-chart" id="vib-chart"></canvas>
            <canvas class="heatmap-container" id="vib-heatmap"></canvas>

            <div class="recommendation-box">
                <strong>💡 建议：</strong> ${result.recommendation}
            </div>
        `;

        drawVibrationChart(result);
        drawVibrationHeatmap(result);
    }

    function drawVibrationChart(result) {
        const canvas = document.getElementById('vib-chart');
        if (!canvas) return;
        const ctx = canvas.getContext('2d');
        const w = canvas.width = canvas.offsetWidth;
        const h = canvas.height = 200;

        ctx.fillStyle = 'rgba(0,0,0,0.3)';
        ctx.fillRect(0, 0, w, h);

        const data = result.time_series || [];
        if (data.length === 0) return;

        const maxVal = Math.max(...data.map(d => d.combined_vibration));
        const minVal = 0;
        const range = maxVal - minVal || 1;

        ctx.strokeStyle = '#3b82f6';
        ctx.lineWidth = 2;
        ctx.beginPath();
        data.forEach((d, i) => {
            const x = (i / data.length) * w;
            const y = h - ((d.combined_vibration - minVal) / range) * (h - 20) - 10;
            if (i === 0) ctx.moveTo(x, y);
            else ctx.lineTo(x, y);
        });
        ctx.stroke();

        ctx.strokeStyle = 'rgba(239, 68, 68, 0.3)';
        ctx.setLineDash([5, 5]);
        ctx.beginPath();
        const resonanceY = h - (1.5 / range) * (h - 20) - 10;
        ctx.moveTo(0, resonanceY);
        ctx.lineTo(w, resonanceY);
        ctx.stroke();
        ctx.setLineDash([]);

        ctx.fillStyle = '#ef4444';
        ctx.font = '12px sans-serif';
        ctx.fillText('共振阈值 (1.5)', 10, resonanceY - 5);
    }

    function drawVibrationHeatmap(result) {
        const canvas = document.getElementById('vib-heatmap');
        if (!canvas) return;
        const ctx = canvas.getContext('2d');
        const w = canvas.width = canvas.offsetWidth;
        const h = canvas.height = 300;

        ctx.fillStyle = 'rgba(0,0,0,0.3)';
        ctx.fillRect(0, 0, w, h);

        const centerX = w / 2;
        const centerY = h / 2;
        const maxDist = Math.min(w, h) / 2 - 30;

        result.device_states.forEach((state, i) => {
            const x = centerX + state.position[0] * 30 + 40;
            const y = centerY + state.position[1] * 30 + 40;

            const gradient = ctx.createRadialGradient(x, y, 0, x, y, state.amplitude * 20);
            gradient.addColorStop(0, 'rgba(233, 69, 96, 0.6)');
            gradient.addColorStop(0.5, 'rgba(233, 69, 96, 0.3)');
            gradient.addColorStop(1, 'rgba(233, 69, 96, 0)');

            ctx.fillStyle = gradient;
            ctx.beginPath();
            ctx.arc(x, y, state.amplitude * 20, 0, Math.PI * 2);
            ctx.fill();

            ctx.fillStyle = '#fff';
            ctx.beginPath();
            ctx.arc(x, y, 8, 0, Math.PI * 2);
            ctx.fill();

            ctx.fillStyle = '#000';
            ctx.font = 'bold 10px sans-serif';
            ctx.textAlign = 'center';
            ctx.fillText(state.device_id.slice(-3), x, y + 4);
        });

        ctx.strokeStyle = 'rgba(255,255,255,0.2)';
        ctx.setLineDash([3, 3]);
        result.device_states.forEach((s1, i) => {
            result.device_states.forEach((s2, j) => {
                if (i < j) {
                    const x1 = centerX + s1.position[0] * 30 + 40;
                    const y1 = centerY + s1.position[1] * 30 + 40;
                    const x2 = centerX + s2.position[0] * 30 + 40;
                    const y2 = centerY + s2.position[1] * 30 + 40;
                    ctx.beginPath();
                    ctx.moveTo(x1, y1);
                    ctx.lineTo(x2, y2);
                    ctx.stroke();
                }
            });
        });
        ctx.setLineDash([]);
    }

    function renderMockVibration(numDevices) {
        const states = [];
        for (let i = 0; i < numDevices; i++) {
            const angle = (i / numDevices) * Math.PI * 2;
            states.push({
                device_id: `shuidui-00${i+1}`,
                phase_offset: angle,
                position: [Math.cos(angle) * 2, Math.sin(angle) * 2],
                frequency: 2.5 + i * 0.3,
                amplitude: 1.2 + i * 0.2,
                foundation_transmitted_amp: 0.5 + i * 0.1
            });
        }

        const timeSeries = [];
        const duration = 5;
        const steps = 500;
        let maxInterference = 0;
        let resonanceCount = 0;
        let total = 0;
        let maxFoundation = 0;

        for (let i = 0; i < steps; i++) {
            const t = (i / steps) * duration;
            let combined = 0;
            let foundationVib = 0;
            states.forEach(s => {
                const phase = 2 * Math.PI * s.frequency * t + s.phase_offset;
                combined += s.amplitude * Math.sin(phase);
                foundationVib += (s.foundation_transmitted_amp || 0) * Math.sin(phase);
            });

            const interference = Math.abs(combined) / states.reduce((a, s) => a + s.amplitude, 0);
            if (interference > maxInterference) maxInterference = interference;
            if (interference > 1.5) resonanceCount++;
            if (Math.abs(foundationVib) > maxFoundation) maxFoundation = Math.abs(foundationVib);
            total += interference;

            timeSeries.push({
                time: t,
                combined_vibration: Math.abs(combined),
                foundation_vibration: Math.abs(foundationVib),
                interference_factor: interference,
                is_resonance: interference > 1.5,
                is_foundation_coupled: Math.abs(foundationVib) > 0.6
            });
        }

        let safetyLevel = '';
        let recommendation = '';
        if (maxInterference < 0.8) { safetyLevel = '安全'; recommendation = '振动干涉在安全范围内，设备可正常运行。'; }
        else if (maxInterference < 1.2) { safetyLevel = '注意'; recommendation = '存在轻度振动干涉，建议监控设备运行状态。'; }
        else if (maxInterference < 1.8) { safetyLevel = '警告'; recommendation = '振动干涉较明显，建议调整设备相位差或增加间隔距离。'; }
        else { safetyLevel = '危险'; recommendation = '存在严重共振风险！请立即调整设备布局或工作相位。'; }

        renderVibrationResults({
            device_states: states,
            time_series: timeSeries,
            max_interference: maxInterference,
            avg_interference: total / steps,
            resonance_count: resonanceCount,
            max_foundation_vibration: maxFoundation,
            foundation_resonance_count: 0,
            safety_level: safetyLevel,
            recommendation: recommendation
        });
    }

    function init() {
        const btn = document.getElementById('btn-analyze-vibration');
        if (btn) btn.addEventListener('click', handleVibrationAnalysis);
    }

    if (typeof document !== 'undefined') {
        if (document.readyState === 'loading') {
            document.addEventListener('DOMContentLoaded', init);
        } else {
            setTimeout(init, 100);
        }
    }

    global.VibrationInterferencePanel = {
        handleVibrationAnalysis,
        renderVibrationResults,
        drawVibrationChart,
        drawVibrationHeatmap,
        setApiBase,
        init
    };

})(typeof window !== 'undefined' ? window : this);
