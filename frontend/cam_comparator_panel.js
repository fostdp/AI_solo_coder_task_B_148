(function (global) {
    'use strict';

    let apiBaseUrl = global._camPanelApiBaseUrl || 'http://localhost:8080/api';
    let currentDeviceId = 'shuidui-001';

    function setApiBase(url) { apiBaseUrl = url; }
    function setCurrentDevice(id) { currentDeviceId = id; }

    const PROFILE_NAMES = {
        cycloidal: { name: '摆线凸轮', eff: 0.85, husk: 0.88, break: 0.08, force: 280, jerk: 1200, cost: 1800 },
        harmonic: { name: '简谐凸轮', eff: 0.78, husk: 0.82, break: 0.06, force: 260, jerk: 800, cost: 1500 },
        trapezoidal: { name: '梯形加速度', eff: 0.82, husk: 0.85, break: 0.07, force: 270, jerk: 500, cost: 2000 },
        polynomial: { name: '3-4-5多项式', eff: 0.88, husk: 0.90, break: 0.05, force: 290, jerk: 600, cost: 2200 },
        involute: { name: '渐开线凸轮', eff: 0.75, husk: 0.80, break: 0.09, force: 250, jerk: 900, cost: 1600 },
        circular_arc: { name: '圆弧凸轮', eff: 0.72, husk: 0.78, break: 0.10, force: 240, jerk: 700, cost: 1400 }
    };

    async function handleCompareProfiles() {
        const selectedTypes = Array.from(document.querySelectorAll('#feature-compare input[type="checkbox"]:checked'))
            .map(cb => cb.value);

        if (selectedTypes.length === 0) {
            alert('请至少选择一种凸轮类型！');
            return;
        }

        if (global.showLoading) global.showLoading('正在进行多凸轮效率对比分析...');

        const request = {
            device_id: currentDeviceId,
            grain_type: document.getElementById('compare-grain').value,
            profile_types: selectedTypes,
            base_radius: parseFloat(document.getElementById('compare-base-radius').value),
            lift: parseFloat(document.getElementById('compare-lift').value)
        };

        try {
            const response = await fetch(`${apiBaseUrl}/compare/cam-profiles`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(request)
            });

            const data = await response.json();

            if (data.success && data.data) {
                renderComparisonResults(data.data);
            } else {
                renderMockComparison(selectedTypes);
            }
        } catch (e) {
            console.error('Comparison failed:', e);
            renderMockComparison(selectedTypes);
        } finally {
            if (global.hideLoading) global.hideLoading();
        }
    }

    function renderComparisonResults(result) {
        const container = document.getElementById('comparison-results');
        container.innerHTML = '';

        result.results.forEach((r, index) => {
            const card = document.createElement('div');
            card.className = `comparison-card ${index === 0 ? 'best' : ''}`;
            card.innerHTML = `
                ${index === 0 ? '<div style="position:absolute;top:-10px;right:10px;background:#4ade80;color:#000;padding:2px 8px;border-radius:12px;font-size:11px;font-weight:bold;">🏆 最优</div>' : ''}
                <h3>${r.profile_name_cn}</h3>
                <div class="score">${(r.score * 100).toFixed(1)}</div>
                <div class="score-label">综合评分</div>
                <div class="metrics">
                    <div class="metric-row">
                        <span class="metric-label">总效率</span>
                        <span class="metric-value">${(r.overall_efficiency * 100).toFixed(1)}%</span>
                    </div>
                    <div class="metric-row">
                        <span class="metric-label">脱壳率</span>
                        <span class="metric-value">${(r.husking_rate * 100).toFixed(1)}%</span>
                    </div>
                    <div class="metric-row">
                        <span class="metric-label">破碎率</span>
                        <span class="metric-value">${(r.breakage_rate * 100).toFixed(1)}%</span>
                    </div>
                    <div class="metric-row">
                        <span class="metric-label">舂捣力</span>
                        <span class="metric-value">${r.pounding_force.toFixed(0)}N</span>
                    </div>
                    <div class="metric-row">
                        <span class="metric-label">最大Jerk</span>
                        <span class="metric-value">${r.max_jerk.toFixed(0)}</span>
                    </div>
                    <div class="metric-row">
                        <span class="metric-label">制造成本</span>
                        <span class="metric-value">¥${r.manufacturing_cost.toFixed(0)}</span>
                    </div>
                </div>
                <canvas class="canvas-preview" id="compare-canvas-${index}"></canvas>
            `;
            container.appendChild(card);
            setTimeout(() => drawMiniProfile(document.getElementById(`compare-canvas-${index}`), r.cam_profile), 0);
        });
    }

    function renderMockComparison(types) {
        const results = types.map(t => ({
            ...PROFILE_NAMES[t],
            profile_type: t,
            score: PROFILE_NAMES[t].eff
        })).sort((a, b) => b.score - a.score);

        const container = document.getElementById('comparison-results');
        container.innerHTML = '';

        results.forEach((r, index) => {
            const card = document.createElement('div');
            card.className = `comparison-card ${index === 0 ? 'best' : ''}`;
            card.style.position = 'relative';
            card.innerHTML = `
                ${index === 0 ? '<div style="position:absolute;top:-10px;right:10px;background:#4ade80;color:#000;padding:2px 8px;border-radius:12px;font-size:11px;font-weight:bold;">🏆 最优</div>' : ''}
                <h3>${r.name}</h3>
                <div class="score">${(r.eff * 100).toFixed(1)}</div>
                <div class="score-label">综合评分</div>
                <div class="metrics">
                    <div class="metric-row">
                        <span class="metric-label">总效率</span>
                        <span class="metric-value">${(r.eff * 100).toFixed(1)}%</span>
                    </div>
                    <div class="metric-row">
                        <span class="metric-label">脱壳率</span>
                        <span class="metric-value">${(r.husk * 100).toFixed(1)}%</span>
                    </div>
                    <div class="metric-row">
                        <span class="metric-label">破碎率</span>
                        <span class="metric-value">${(r.break * 100).toFixed(1)}%</span>
                    </div>
                    <div class="metric-row">
                        <span class="metric-label">舂捣力</span>
                        <span class="metric-value">${r.force.toFixed(0)}N</span>
                    </div>
                    <div class="metric-row">
                        <span class="metric-label">最大Jerk</span>
                        <span class="metric-value">${r.jerk.toFixed(0)}</span>
                    </div>
                    <div class="metric-row">
                        <span class="metric-label">制造成本</span>
                        <span class="metric-value">¥${r.cost.toFixed(0)}</span>
                    </div>
                </div>
            `;
            container.appendChild(card);
        });
    }

    function drawMiniProfile(canvas, profile) {
        if (!canvas) return;
        const ctx = canvas.getContext('2d');
        const w = canvas.width = canvas.offsetWidth;
        const h = canvas.height = 100;

        ctx.fillStyle = 'rgba(0,0,0,0.3)';
        ctx.fillRect(0, 0, w, h);

        if (!profile || !profile.length) return;

        const maxR = Math.max(...profile.map(p => p.radius));
        const minR = Math.min(...profile.map(p => p.radius));
        const range = maxR - minR || 1;

        ctx.strokeStyle = '#e94560';
        ctx.lineWidth = 2;
        ctx.beginPath();
        profile.forEach((p, i) => {
            const x = (i / profile.length) * w;
            const y = h - ((p.radius - minR) / range) * (h - 20) - 10;
            if (i === 0) ctx.moveTo(x, y);
            else ctx.lineTo(x, y);
        });
        ctx.stroke();
    }

    function init() {
        const btn = document.getElementById('btn-compare');
        if (btn) btn.addEventListener('click', handleCompareProfiles);
    }

    if (typeof document !== 'undefined') {
        if (document.readyState === 'loading') {
            document.addEventListener('DOMContentLoaded', init);
        } else {
            setTimeout(init, 100);
        }
    }

    global.CamComparatorPanel = {
        handleCompareProfiles,
        renderComparisonResults,
        drawMiniProfile,
        setApiBase,
        setCurrentDevice,
        init
    };

})(typeof window !== 'undefined' ? window : this);
