(function (global) {
    'use strict';

    let forceHistory = [];
    let currentTab = 'profile';
    let currentDeviceId = 'shuidui-001';
    let camProfileData = [];
    let deviceParams = { cam_base_radius: 0.15, cam_lift: 0.12, duitou_mass: 25 };
    let chartUpdateCallback = null;
    let apiBaseUrl = 'http://localhost:8080/api';
    let retryCount = 0;
    let maxForceHistory = 50;

    function initCamPanel() {
        setupEventListeners();
        loadCamProfile();
        connectWebSocket();
        startDataPolling();
    }

    function setupEventListeners() {
        document.getElementById('device-select').addEventListener('change', (e) => {
            currentDeviceId = e.target.value;
            loadCamProfile();
            forceHistory = [];
        });

        document.querySelectorAll('.tab-btn').forEach(btn => {
            btn.addEventListener('click', () => {
                document.querySelectorAll('.tab-btn').forEach(b => b.classList.remove('active'));
                btn.classList.add('active');
                currentTab = btn.dataset.tab;
                drawCamProfile();
            });
        });

        document.getElementById('btn-optimize').addEventListener('click', handleOptimize);
    }

    function updateSensorDisplay(sensor) {
        const el = (id, val) => { const e = document.getElementById(id); if (e) e.textContent = val; };
        el('cam-angle-display', sensor.cam_angle.toFixed(1) + '°');
        el('duitou-position', sensor.duitou_position.toFixed(3) + ' m');
        el('wheel-speed', sensor.water_wheel_speed.toFixed(2) + ' rad/s');
        el('vibration-value', sensor.frame_vibration_total.toFixed(2));

        const vibCard = document.getElementById('vibration-card');
        if (vibCard) {
            vibCard.classList.remove('warning', 'critical');
            if (sensor.frame_vibration_total > 8) {
                vibCard.classList.add('critical');
            } else if (sensor.frame_vibration_total > 5) {
                vibCard.classList.add('warning');
            }
        }
    }

    function updateDynamicsDisplay(dynamics) {
        const el = (id, val) => { const e = document.getElementById(id); if (e) e.textContent = val; };
        if (dynamics && dynamics.pounding_force !== undefined) {
            el('pounding-force', dynamics.pounding_force.toFixed(0));
            el('impact-energy', dynamics.impact_energy.toFixed(2));

            const huskingRate = dynamics.husking_rate !== undefined ? dynamics.husking_rate : 0.7 + Math.random() * 0.2;
            const breakageRate = dynamics.breakage_rate !== undefined ? dynamics.breakage_rate : 0.05 + Math.random() * 0.08;
            el('husking-rate', (huskingRate * 100).toFixed(1));
            el('breakage-rate', (breakageRate * 100).toFixed(1));

            forceHistory.push({
                time: Date.now(),
                force: dynamics.pounding_force
            });
            if (forceHistory.length > maxForceHistory) forceHistory.shift();
        }
    }

    function addForceValue(force) {
        forceHistory.push({
            time: Date.now(),
            force: force
        });
        if (forceHistory.length > maxForceHistory) forceHistory.shift();
    }

    let camCtx, camCanvas;

    function initCamCanvas() {
        camCanvas = document.getElementById('cam-canvas');
        if (!camCanvas) return;
        camCtx = camCanvas.getContext('2d');
        resizeCamCanvas();
        window.addEventListener('resize', resizeCamCanvas);
    }

    function resizeCamCanvas() {
        if (!camCanvas) return;
        const rect = camCanvas.parentElement.getBoundingClientRect();
        const dpr = window.devicePixelRatio || 1;
        camCanvas.width = (rect.width - 30) * dpr;
        camCanvas.height = 180 * dpr;
        camCanvas.style.width = (rect.width - 30) + 'px';
        camCanvas.style.height = '180px';
        camCtx.scale(dpr, dpr);
        drawCamProfile();
    }

    function drawCamProfile() {
        if (!camCanvas || !camCtx) {
            initCamCanvas();
            if (!camCanvas || !camCtx) return;
        }

        const width = camCanvas.width / (window.devicePixelRatio || 1);
        const height = camCanvas.height / (window.devicePixelRatio || 1);

        camCtx.clearRect(0, 0, width, height);

        const centerX = width / 2;
        const centerY = height / 2;
        const maxRadius = Math.min(width, height) * 0.4;

        camCtx.strokeStyle = 'rgba(255, 255, 255, 0.1)';
        camCtx.lineWidth = 1;
        for (let i = 1; i <= 3; i++) {
            camCtx.beginPath();
            camCtx.arc(centerX, centerY, maxRadius * i / 3, 0, Math.PI * 2);
            camCtx.stroke();
        }

        for (let i = 0; i < 12; i++) {
            const angle = (i / 12) * Math.PI * 2;
            camCtx.beginPath();
            camCtx.moveTo(centerX, centerY);
            camCtx.lineTo(
                centerX + Math.cos(angle) * maxRadius,
                centerY + Math.sin(angle) * maxRadius
            );
            camCtx.stroke();
        }

        if (camProfileData.length === 0) {
            camCtx.fillStyle = '#888';
            camCtx.font = '14px sans-serif';
            camCtx.textAlign = 'center';
            camCtx.fillText('加载中...', centerX, centerY);
            return;
        }

        let maxValue = 0;
        camProfileData.forEach(p => {
            let val;
            if (currentTab === 'profile') val = p.lift;
            else if (currentTab === 'velocity') val = Math.abs(p.velocity);
            else val = Math.abs(p.acceleration);
            if (val > maxValue) maxValue = val;
        });

        const baseRadius = 0.3 * maxRadius;

        camCtx.beginPath();
        camProfileData.forEach((p, i) => {
            const angle = p.angle * Math.PI / 180 - Math.PI / 2;
            let value;
            if (currentTab === 'profile') {
                value = baseRadius + (p.lift / maxValue) * maxRadius * 0.7;
            } else if (currentTab === 'velocity') {
                value = baseRadius + (Math.abs(p.velocity) / maxValue) * maxRadius * 0.7;
            } else {
                value = baseRadius + (Math.abs(p.acceleration) / maxValue) * maxRadius * 0.7;
            }

            const x = centerX + Math.cos(angle) * value;
            const y = centerY + Math.sin(angle) * value;

            if (i === 0) {
                camCtx.moveTo(x, y);
            } else {
                camCtx.lineTo(x, y);
            }
        });
        camCtx.closePath();

        const gradient = camCtx.createRadialGradient(centerX, centerY, 0, centerX, centerY, maxRadius);
        gradient.addColorStop(0, 'rgba(233, 69, 96, 0.1)');
        gradient.addColorStop(1, 'rgba(233, 69, 96, 0.3)');
        camCtx.fillStyle = gradient;
        camCtx.fill();

        camCtx.strokeStyle = '#e94560';
        camCtx.lineWidth = 2;
        camCtx.stroke();

        camCtx.fillStyle = '#aaa';
        camCtx.font = '12px sans-serif';
        camCtx.textAlign = 'left';
        camCtx.fillText(`最大值: ${maxValue.toFixed(4)}`, 10, 20);
    }

    let forceCtx, forceCanvas;

    function initForceChart() {
        forceCanvas = document.getElementById('force-chart');
        if (!forceCanvas) return;
        forceCtx = forceCanvas.getContext('2d');
        resizeForceChart();
        window.addEventListener('resize', resizeForceChart);
    }

    function resizeForceChart() {
        if (!forceCanvas) return;
        const rect = forceCanvas.parentElement.getBoundingClientRect();
        const dpr = window.devicePixelRatio || 1;
        forceCanvas.width = rect.width * dpr;
        forceCanvas.height = 200 * dpr;
        forceCanvas.style.width = rect.width + 'px';
        forceCanvas.style.height = '200px';
        forceCtx.scale(dpr, dpr);
    }

    function drawForceChart() {
        if (!forceCanvas || !forceCtx) {
            initForceChart();
            if (!forceCanvas || !forceCtx) return;
        }

        const width = forceCanvas.width / (window.devicePixelRatio || 1);
        const height = forceCanvas.height / (window.devicePixelRatio || 1);

        forceCtx.clearRect(0, 0, width, height);

        forceCtx.fillStyle = 'rgba(0, 0, 0, 0.2)';
        forceCtx.fillRect(0, 0, width, height);

        forceCtx.strokeStyle = 'rgba(255, 255, 255, 0.05)';
        forceCtx.lineWidth = 1;
        for (let i = 0; i <= 5; i++) {
            const y = height * i / 5;
            forceCtx.beginPath();
            forceCtx.moveTo(0, y);
            forceCtx.lineTo(width, y);
            forceCtx.stroke();
        }

        if (forceHistory.length < 2) return;

        const maxForce = Math.max(...forceHistory.map(d => d.force), 100);
        const barWidth = width / maxForceHistory - 2;

        forceHistory.forEach((d, i) => {
            const x = i * (width / maxForceHistory);
            const barHeight = (d.force / maxForce) * (height - 30);
            const y = height - barHeight - 10;

            const gradient = forceCtx.createLinearGradient(x, y, x, height - 10);
            if (d.force > 800) {
                gradient.addColorStop(0, '#ef4444');
                gradient.addColorStop(1, '#7f1d1d');
            } else if (d.force > 400) {
                gradient.addColorStop(0, '#f59e0b');
                gradient.addColorStop(1, '#92400e');
            } else {
                gradient.addColorStop(0, '#4ade80');
                gradient.addColorStop(1, '#166534');
            }

            forceCtx.fillStyle = gradient;
            forceCtx.fillRect(x + 1, y, barWidth, barHeight);
        });

        forceCtx.fillStyle = '#888';
        forceCtx.font = '11px sans-serif';
        forceCtx.textAlign = 'left';
        forceCtx.fillText(`${maxForce.toFixed(0)} N`, 5, 15);
        forceCtx.fillText('0 N', 5, height - 5);
    }

    async function loadCamProfile() {
        try {
            const response = await fetch(`${apiBaseUrl}/cam-profile/${currentDeviceId}`);
            const data = await response.json();
            if (data.success && data.data) {
                camProfileData = data.data;
                drawCamProfile();
            }
        } catch (e) {
            console.error('Failed to load cam profile:', e);
            camProfileData = generateMockCamProfile();
            drawCamProfile();
        }
    }

    function generateMockCamProfile() {
        const points = [];
        const lift = 0.12;
        for (let i = 0; i < 360; i++) {
            const angle = i;
            const angleRad = angle * Math.PI / 180;
            let liftVal, velocity, acceleration;

            if (angleRad < Math.PI) {
                const t = angleRad / Math.PI;
                liftVal = lift * (1 - Math.cos(Math.PI * t)) / 2;
                velocity = lift * Math.PI * Math.sin(Math.PI * t) / 2;
                acceleration = lift * Math.PI * Math.PI * Math.cos(Math.PI * t) / 2;
            } else {
                const t = (angleRad - Math.PI) / Math.PI;
                liftVal = lift * (1 + Math.cos(Math.PI * t)) / 2;
                velocity = -lift * Math.PI * Math.sin(Math.PI * t) / 2;
                acceleration = -lift * Math.PI * Math.PI * Math.cos(Math.PI * t) / 2;
            }

            points.push({
                angle,
                radius: 0.15 + liftVal,
                lift: liftVal,
                velocity,
                acceleration
            });
        }
        return points;
    }

    function addAlert(alert) {
        const list = document.getElementById('alerts-list');
        if (!list) return;

        if (list.querySelector('.text-center, [style*="text-align: center"]')) {
            list.innerHTML = '';
        }

        const item = document.createElement('div');
        item.className = `alert-item ${alert.alert_level || 'warning'}`;

        const time = new Date(alert.timestamp || Date.now()).toLocaleTimeString();

        item.innerHTML = `
            <div class="alert-header">
                <span class="alert-type">${alert.alert_type || '未知告警'}</span>
                <span class="alert-time">${time}</span>
            </div>
            <div class="alert-msg">${alert.alert_message || ''}</div>
        `;

        list.insertBefore(item, list.firstChild);

        while (list.children.length > 20) {
            list.removeChild(list.lastChild);
        }

        if (alert.alert_level === 'critical' || alert.alert_level === 'Critical') {
            document.getElementById('vibration-card')?.classList.add('critical');
        }
    }

    function connectWebSocket() {
        const ws = new WebSocket('ws://localhost:8080/ws/alerts');
        ws.onopen = () => {
            console.log('WebSocket connected');
            updateConnectionStatus('connected');
        };
        ws.onmessage = (event) => {
            try {
                const alert = JSON.parse(event.data);
                addAlert(alert);
                retryCount = 0;
            } catch (e) { console.error(e); }
        };
        ws.onclose = () => {
            console.log('WebSocket disconnected');
            updateConnectionStatus('disconnected');
            retryCount = (retryCount || 0) + 1;
            const delay = Math.min(3000 * Math.pow(1.5, retryCount - 1), 30000);
            setTimeout(connectWebSocket, delay);
        };
        ws.onerror = () => {
            updateConnectionStatus('disconnected');
        };
    }

    function updateConnectionStatus(status) {
        const dot = document.getElementById('connection-status');
        const text = document.getElementById('connection-text');

        if (status === 'connected') {
            if (dot) dot.className = 'status-dot';
            if (text) text.textContent = '系统运行中';
        } else {
            if (dot) dot.className = 'status-dot warning';
            if (text) text.textContent = '连接断开';
        }
    }

    let isOptimizing = false;

    async function handleOptimize() {
        if (isOptimizing) return;

        isOptimizing = true;
        const btn = document.getElementById('btn-optimize');
        btn.textContent = '优化中...';
        btn.disabled = true;

        const request = {
            device_id: currentDeviceId,
            target_efficiency: parseFloat(document.getElementById('opt-target-eff').value),
            grain_type: document.getElementById('grain-type').value,
            constraints: {
                max_cam_radius: parseFloat(document.getElementById('opt-base-radius').value) * 1.5,
                min_cam_radius: 0.05,
                max_lift: parseFloat(document.getElementById('opt-lift').value) * 1.5,
                max_pressure_angle: parseFloat(document.getElementById('opt-pressure-angle').value) * Math.PI / 180
            }
        };

        try {
            const response = await fetch(`${apiBaseUrl}/optimize`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(request)
            });

            const data = await response.json();

            if (data.success && data.data) {
                const result = data.data;
                document.getElementById('opt-results').style.display = 'grid';
                document.getElementById('opt-efficiency').textContent = ((result.overall_efficiency || result.actual_efficiency || 0) * 100).toFixed(1) + '%';
                document.getElementById('opt-profile-type').textContent = getProfileTypeName(result.cam_profile_type);
                document.getElementById('opt-base-result').textContent = (result.base_radius || result.cam_base_radius || 0).toFixed(3) + 'm';
                document.getElementById('opt-lift-result').textContent = (result.lift || result.cam_lift || 0).toFixed(3) + 'm';

                if (result.cam_profile || result.cam_profile_points) {
                    camProfileData = result.cam_profile || result.cam_profile_points;
                    drawCamProfile();
                }
            }
        } catch (e) {
            console.error('Optimization failed:', e);
            const mockResult = mockOptimization();
            document.getElementById('opt-results').style.display = 'grid';
            document.getElementById('opt-efficiency').textContent = (mockResult.efficiency * 100).toFixed(1) + '%';
            document.getElementById('opt-profile-type').textContent = '摆线凸轮';
            document.getElementById('opt-base-result').textContent = mockResult.baseRadius.toFixed(3) + 'm';
            document.getElementById('opt-lift-result').textContent = mockResult.lift.toFixed(3) + 'm';
        }

        isOptimizing = false;
        btn.textContent = '开始优化';
        btn.disabled = false;
    }

    function getProfileTypeName(type) {
        const names = {
            'cycloidal': '摆线凸轮',
            'harmonic': '简谐凸轮',
            'trapezoidal': '梯形加速度凸轮',
            'polynomial': '多项式凸轮'
        };
        return names[type] || type;
    }

    function mockOptimization() {
        return {
            efficiency: 0.87,
            baseRadius: 0.18,
            lift: 0.14
        };
    }

    function startDataPolling() {
        setInterval(() => {
            fetchLatestData();
        }, 1000);
    }

    async function fetchLatestData() {
        try {
            const response = await fetch(`${apiBaseUrl}/sensor-data?device_id=${currentDeviceId}&limit=1`);
            const data = await response.json();
            if (data.success && data.data && data.data.length > 0) {
                updateSensorDisplay(data.data[0]);
            }
        } catch (e) {
        }
    }

    function setChartUpdateCallback(cb) {
        chartUpdateCallback = cb;
    }

    function triggerChartUpdate() {
        if (chartUpdateCallback) chartUpdateCallback();
    }

    global.initCamPanel = initCamPanel;
    global.initCamCanvas = initCamCanvas;
    global.initForceChart = initForceChart;
    global.drawCamProfile = drawCamProfile;
    global.drawForceChart = drawForceChart;
    global.loadCamProfile = loadCamProfile;
    global.updateSensorDisplay = updateSensorDisplay;
    global.updateDynamicsDisplay = updateDynamicsDisplay;
    global.addForceValue = addForceValue;
    global.handleOptimize = handleOptimize;
    global.connectWebSocket = connectWebSocket;
    global.setChartUpdateCallback = setChartUpdateCallback;
    global.triggerChartUpdate = triggerChartUpdate;
    global.setApiBaseUrl = (url) => { apiBaseUrl = url; };

    Object.defineProperty(global, 'forceHistory', { get: () => forceHistory });
    Object.defineProperty(global, 'currentTab', { get: () => currentTab });
    Object.defineProperty(global, 'camProfileData', { get: () => camProfileData });
    Object.defineProperty(global, 'currentDeviceId', { get: () => currentDeviceId });
    Object.defineProperty(global, 'deviceParams', { get: () => deviceParams });

    // ============ 新功能：选项卡切换 ============
    function initFeatureTabs() {
        document.querySelectorAll('.feature-tab-btn').forEach(btn => {
            btn.addEventListener('click', () => {
                const feature = btn.dataset.feature;
                document.querySelectorAll('.feature-tab-btn').forEach(b => b.classList.remove('active'));
                btn.classList.add('active');
                document.querySelectorAll('.feature-panel .tab-content').forEach(c => c.classList.remove('active'));
                document.getElementById(`feature-${feature}`).classList.add('active');
            });
        });
    }

    function showLoading(text = '处理中...') {
        document.getElementById('loading-text').textContent = text;
        document.getElementById('loading-overlay').classList.add('active');
    }

    function hideLoading() {
        document.getElementById('loading-overlay').classList.remove('active');
    }

    // ============ 功能1：凸轮效率对比 ============
    async function handleCompareProfiles() {
        const selectedTypes = Array.from(document.querySelectorAll('#feature-compare input[type="checkbox"]:checked'))
            .map(cb => cb.value);
        
        if (selectedTypes.length === 0) {
            alert('请至少选择一种凸轮类型！');
            return;
        }

        showLoading('正在进行多凸轮效率对比分析...');

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
            hideLoading();
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
        const names = {
            cycloidal: { name: '摆线凸轮', eff: 0.85, husk: 0.88, break: 0.08, force: 280, jerk: 1200, cost: 1800 },
            harmonic: { name: '简谐凸轮', eff: 0.78, husk: 0.82, break: 0.06, force: 260, jerk: 800, cost: 1500 },
            trapezoidal: { name: '梯形加速度', eff: 0.82, husk: 0.85, break: 0.07, force: 270, jerk: 500, cost: 2000 },
            polynomial: { name: '3-4-5多项式', eff: 0.88, husk: 0.90, break: 0.05, force: 290, jerk: 600, cost: 2200 },
            involute: { name: '渐开线凸轮', eff: 0.75, husk: 0.80, break: 0.09, force: 250, jerk: 900, cost: 1600 },
            circular_arc: { name: '圆弧凸轮', eff: 0.72, husk: 0.78, break: 0.10, force: 240, jerk: 700, cost: 1400 }
        };

        const results = types.map(t => ({
            ...names[t],
            profile_type: t,
            score: names[t].eff
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

    // ============ 功能2：跨时代效率对比 ============
    async function handleCrossEraComparison() {
        showLoading('正在进行跨时代效率对比...');

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
            hideLoading();
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

    // ============ 功能3：振动干涉分析 ============
    async function handleVibrationAnalysis() {
        const selectedDevices = Array.from(document.querySelectorAll('#feature-vibration input[type="checkbox"]:checked'))
            .map(cb => cb.value);
        
        if (selectedDevices.length < 2) {
            alert('请至少选择2台或更多水碓进行干涉分析！');
            return;
        }

        showLoading('正在进行多台水碓振动干涉分析...');

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
            hideLoading();
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
            const angle = (i / result.device_states.length) * Math.PI * 2;
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
                amplitude: 1.2 + i * 0.2
            });
        }

        const timeSeries = [];
        const duration = 5;
        const steps = 500;
        let maxInterference = 0;
        let resonanceCount = 0;
        let total = 0;

        for (let i = 0; i < steps; i++) {
            const t = (i / steps) * duration;
            let combined = 0;
            states.forEach(s => {
                const phase = 2 * Math.PI * s.frequency * t + s.phase_offset;
                combined += s.amplitude * Math.sin(phase);
            });

            const interference = Math.abs(combined) / states.reduce((a, s) => a + s.amplitude, 0);
            if (interference > maxInterference) maxInterference = interference;
            if (interference > 1.5) resonanceCount++;
            total += interference;

            timeSeries.push({
                time: t,
                combined_vibration: Math.abs(combined),
                interference_factor: interference,
                is_resonance: interference > 1.5
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
            safety_level: safetyLevel,
            recommendation: recommendation
        });
    }

    // ============ 功能4：虚拟凸轮设计（简化版：控制点+Bezier+预设模板） ============
    let drawingCanvas = null;
    let drawingCtx = null;
    let controlPoints = [];
    let draggingIdx = -1;
    let currentTemplate = 'harmonic';

    function initDrawingCanvas() {
        drawingCanvas = document.getElementById('drawing-canvas');
        if (!drawingCanvas) return;
        drawingCtx = drawingCanvas.getContext('2d');
        drawingCanvas.width = drawingCanvas.offsetWidth;
        drawingCanvas.height = 300;

        applyTemplate(currentTemplate);

        drawingCanvas.addEventListener('mousedown', onCanvasMouseDown);
        drawingCanvas.addEventListener('mousemove', onCanvasMouseMove);
        drawingCanvas.addEventListener('mouseup', onCanvasMouseUp);
        drawingCanvas.addEventListener('mouseleave', onCanvasMouseUp);

        drawingCanvas.addEventListener('touchstart', onCanvasTouchStart);
        drawingCanvas.addEventListener('touchmove', onCanvasTouchMove);
        drawingCanvas.addEventListener('touchend', onCanvasMouseUp);

        document.querySelectorAll('.tpl-btn').forEach(btn => {
            btn.addEventListener('click', () => {
                document.querySelectorAll('.tpl-btn').forEach(b => b.classList.remove('active'));
                btn.classList.add('active');
                currentTemplate = btn.dataset.tpl;
                applyTemplate(currentTemplate);
            });
        });
    }

    function applyTemplate(tpl) {
        const w = drawingCanvas.width;
        const h = drawingCanvas.height;
        const yBase = h * 0.7;
        const yTop = h * 0.3;
        const controlCount = 6;

        controlPoints = [];
        for (let i = 0; i < controlCount; i++) {
            const t = i / (controlCount - 1);
            const x = t * w;
            let normalizedLift;

            if (tpl === 'harmonic') {
                if (t < 0.5) normalizedLift = 0.5 * (1 - Math.cos(Math.PI * t * 2));
                else normalizedLift = 0.5 * (1 + Math.cos(Math.PI * (t - 0.5) * 2));
            } else if (tpl === 'cycloidal') {
                if (t < 0.5) normalizedLift = t * 2 - Math.sin(2 * Math.PI * t) / (2 * Math.PI);
                else {
                    const tt = (t - 0.5) * 2;
                    normalizedLift = 1 - (tt - Math.sin(2 * Math.PI * tt) / (2 * Math.PI));
                }
            } else if (tpl === 'polynomial') {
                if (t < 0.5) {
                    const tt = t * 2;
                    normalizedLift = 10 * Math.pow(tt, 3) - 15 * Math.pow(tt, 4) + 6 * Math.pow(tt, 5);
                } else {
                    const tt = (t - 0.5) * 2;
                    normalizedLift = 1 - (10 * Math.pow(tt, 3) - 15 * Math.pow(tt, 4) + 6 * Math.pow(tt, 5));
                }
            } else if (tpl === 'arc') {
                if (t < 0.5) {
                    normalizedLift = 1 - Math.sqrt(1 - Math.pow(t * 2, 2));
                } else {
                    normalizedLift = 1 - Math.sqrt(1 - Math.pow((1 - t) * 2, 2));
                }
            } else {
                normalizedLift = Math.sin(t * Math.PI);
            }

            normalizedLift = Math.max(0, Math.min(1, normalizedLift));
            const y = yBase - normalizedLift * (yBase - yTop);
            controlPoints.push({ x, y });
        }

        redrawCanvas();
    }

    function clearDrawingCanvas() {
        if (!drawingCtx) return;
        const w = drawingCanvas.width;
        const h = drawingCanvas.height;

        drawingCtx.fillStyle = 'rgba(0,0,0,0.3)';
        drawingCtx.fillRect(0, 0, w, h);

        drawingCtx.strokeStyle = 'rgba(255,255,255,0.1)';
        drawingCtx.lineWidth = 1;
        for (let i = 0; i <= 10; i++) {
            const y = (h / 10) * i;
            drawingCtx.beginPath();
            drawingCtx.moveTo(0, y);
            drawingCtx.lineTo(w, y);
            drawingCtx.stroke();
        }

        drawingCtx.strokeStyle = 'rgba(233,69,96,0.3)';
        drawingCtx.lineWidth = 2;
        drawingCtx.setLineDash([5, 5]);
        drawingCtx.beginPath();
        drawingCtx.moveTo(0, h * 0.7);
        drawingCtx.lineTo(w, h * 0.3);
        drawingCtx.stroke();
        drawingCtx.setLineDash([]);

        drawingCtx.fillStyle = '#888';
        drawingCtx.font = '12px sans-serif';
        drawingCtx.fillText('0°', 5, h - 5);
        drawingCtx.fillText('180°', w / 2 - 15, h - 5);
        drawingCtx.fillText('360°', w - 25, h - 5);
        drawingCtx.fillText('最大升程', 5, 15);
        drawingCtx.fillText('起始位置', 5, h * 0.7 + 15);
    }

    function bezierPoint(p0, p1, p2, t) {
        const u = 1 - t;
        return {
            x: u * u * p0.x + 2 * u * t * p1.x + t * t * p2.x,
            y: u * u * p0.y + 2 * u * t * p1.y + t * t * p2.y
        };
    }

    function redrawCanvas() {
        clearDrawingCanvas();
        if (controlPoints.length < 2) return;

        drawingCtx.strokeStyle = '#4ade80';
        drawingCtx.lineWidth = 3;
        drawingCtx.lineCap = 'round';
        drawingCtx.lineJoin = 'round';
        drawingCtx.beginPath();

        const samplesPerSeg = 20;
        for (let i = 0; i < controlPoints.length - 1; i++) {
            const p0 = controlPoints[i];
            const p2 = controlPoints[i + 1];
            const p1 = {
                x: (p0.x + p2.x) / 2,
                y: (p0.y + p2.y) / 2
            };
            for (let s = 0; s <= samplesPerSeg; s++) {
                const t = s / samplesPerSeg;
                const pt = bezierPoint(p0, p1, p2, t);
                if (i === 0 && s === 0) drawingCtx.moveTo(pt.x, pt.y);
                else drawingCtx.lineTo(pt.x, pt.y);
            }
        }
        drawingCtx.stroke();

        drawingCtx.strokeStyle = 'rgba(251, 191, 36, 0.3)';
        drawingCtx.lineWidth = 1;
        drawingCtx.setLineDash([4, 4]);
        drawingCtx.beginPath();
        controlPoints.forEach((p, i) => {
            if (i === 0) drawingCtx.moveTo(p.x, p.y);
            else drawingCtx.lineTo(p.x, p.y);
        });
        drawingCtx.stroke();
        drawingCtx.setLineDash([]);

        controlPoints.forEach((p, i) => {
            drawingCtx.fillStyle = i === 0 || i === controlPoints.length - 1 ? '#94a3b8' : '#fbbf24';
            drawingCtx.beginPath();
            drawingCtx.arc(p.x, p.y, 7, 0, Math.PI * 2);
            drawingCtx.fill();
            drawingCtx.strokeStyle = '#000';
            drawingCtx.lineWidth = 2;
            drawingCtx.stroke();
        });
    }

    function getCanvasCoords(e) {
        const rect = drawingCanvas.getBoundingClientRect();
        return {
            x: e.clientX - rect.left,
            y: e.clientY - rect.top
        };
    }

    function findNearestControlPoint(coords) {
        let idx = -1, minDist = 25;
        for (let i = 1; i < controlPoints.length - 1; i++) {
            const dx = coords.x - controlPoints[i].x;
            const dy = coords.y - controlPoints[i].y;
            const d = Math.sqrt(dx * dx + dy * dy);
            if (d < minDist) {
                minDist = d;
                idx = i;
            }
        }
        return idx;
    }

    function onCanvasMouseDown(e) {
        const coords = getCanvasCoords(e);
        draggingIdx = findNearestControlPoint(coords);
        if (draggingIdx >= 0) {
            drawingCanvas.style.cursor = 'grabbing';
        }
    }

    function onCanvasMouseMove(e) {
        const coords = getCanvasCoords(e);
        if (draggingIdx >= 0) {
            const w = drawingCanvas.width;
            const h = drawingCanvas.height;
            controlPoints[draggingIdx].x = Math.max(0, Math.min(w, coords.x));
            controlPoints[draggingIdx].y = Math.max(h * 0.1, Math.min(h * 0.9, coords.y));
            redrawCanvas();
        } else {
            const near = findNearestControlPoint(coords);
            drawingCanvas.style.cursor = near >= 0 ? 'grab' : 'crosshair';
        }
    }

    function onCanvasMouseUp() {
        draggingIdx = -1;
        drawingCanvas.style.cursor = 'crosshair';
    }

    function onCanvasTouchStart(e) {
        e.preventDefault();
        const touch = e.touches[0];
        onCanvasMouseDown({ clientX: touch.clientX, clientY: touch.clientY });
    }

    function onCanvasTouchMove(e) {
        e.preventDefault();
        if (draggingIdx < 0) return;
        const touch = e.touches[0];
        onCanvasMouseMove({ clientX: touch.clientX, clientY: touch.clientY });
    }

    function sampleLiftFromCurve() {
        if (controlPoints.length < 2) return null;
        const w = drawingCanvas.width;
        const h = drawingCanvas.height;
        const numPoints = 72;
        const lifts = [];
        const yBase = h * 0.7;
        const yTop = h * 0.3;
        const totalH = Math.max(0.001, yBase - yTop);

        for (let i = 0; i < numPoints; i++) {
            const targetX = (i / numPoints) * w;
            let nearestY = yBase;

            for (let j = 0; j < controlPoints.length - 1; j++) {
                const p0 = controlPoints[j];
                const p2 = controlPoints[j + 1];
                const p1 = { x: (p0.x + p2.x) / 2, y: (p0.y + p2.y) / 2 };
                if (targetX >= p0.x - 0.5 && targetX <= p2.x + 0.5) {
                    let bestT = 0, bestDiff = Infinity;
                    for (let s = 0; s <= 40; s++) {
                        const t = s / 40;
                        const pt = bezierPoint(p0, p1, p2, t);
                        const diff = Math.abs(pt.x - targetX);
                        if (diff < bestDiff) { bestDiff = diff; bestT = t; }
                    }
                    const best = bezierPoint(p0, p1, p2, bestT);
                    nearestY = best.y;
                    break;
                }
            }

            const normalized = Math.max(0, Math.min(1, (yBase - nearestY) / totalH));
            lifts.push(normalized * 0.12);
        }
        return lifts;
    }

    async function handleTestDesign() {
        const lifts = sampleLiftFromCurve();
        if (!lifts) {
            alert('请先选择预设曲线或调整控制点！');
            return;
        }

        showLoading('正在分析您的凸轮设计...');

        const request = {
            user_id: null,
            design_name: document.getElementById('design-name').value || '用户设计',
            base_radius: 0.15,
            grain_type: document.getElementById('design-grain').value,
            user_defined_lifts: lifts
        };

        try {
            const response = await fetch(`${apiBaseUrl}/user-cam/test`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(request)
            });

            const data = await response.json();

            if (data.success && data.data) {
                renderDesignResults(data.data);
            } else {
                renderMockDesign();
            }
        } catch (e) {
            console.error('User cam test failed:', e);
            renderMockDesign();
        } finally {
            hideLoading();
        }
    }

    function renderDesignResults(result) {
        const container = document.getElementById('design-results');

        const gradeClass = `grade-${result.grade.charAt(0).toLowerCase()}`;

        container.innerHTML = `
            <div class="grade-display">
                <div class="grade-badge ${gradeClass}">${result.grade}</div>
                <div style="margin-top:10px;font-size:14px;color:#888;">综合评分: ${(result.overall_score * 100).toFixed(1)}</div>
            </div>

            <div class="result-preview">
                <div class="preview-section">
                    <h4>📊 性能指标</h4>
                    <div class="stats-grid">
                        <div class="stat-item">
                            <div class="stat-label">总效率</div>
                            <div class="stat-value">${(result.overall_efficiency * 100).toFixed(1)}%</div>
                        </div>
                        <div class="stat-item">
                            <div class="stat-label">脱壳率</div>
                            <div class="stat-value">${(result.husking_rate * 100).toFixed(1)}%</div>
                        </div>
                        <div class="stat-item">
                            <div class="stat-label">破碎率</div>
                            <div class="stat-value">${(result.breakage_rate * 100).toFixed(1)}%</div>
                        </div>
                        <div class="stat-item">
                            <div class="stat-label">舂捣力</div>
                            <div class="stat-value">${result.pounding_force.toFixed(0)}N</div>
                        </div>
                    </div>
                </div>
                <div class="preview-section">
                    <h4>🔧 公差分析</h4>
                    <div class="stats-grid">
                        <div class="stat-item">
                            <div class="stat-label">最小曲率</div>
                            <div class="stat-value">${result.tolerance_report.min_curvature.toFixed(4)}m</div>
                        </div>
                        <div class="stat-item">
                            <div class="stat-label">加工可行性</div>
                            <div class="stat-value">${(result.tolerance_report.overall_feasibility * 100).toFixed(1)}%</div>
                        </div>
                        <div class="stat-item">
                            <div class="stat-label">制造成本</div>
                            <div class="stat-value">¥${result.tolerance_report.manufacturing_cost.toFixed(0)}</div>
                        </div>
                        <div class="stat-item">
                            <div class="stat-label">冲击能量</div>
                            <div class="stat-value">${result.impact_energy.toFixed(2)}J</div>
                        </div>
                    </div>
                </div>
            </div>

            <div class="feedback-list">
                <h4 style="color:#f39c12;margin-top:15px;font-size:14px;">💡 设计反馈</h4>
                ${result.design_feedback.map(f => `<div class="feedback-item positive">✓ ${f}</div>`).join('')}
                ${result.safety_warnings.map(w => `<div class="feedback-item warning">⚠ ${w}</div>`).join('')}
            </div>
        `;
    }

    function renderMockDesign() {
        renderDesignResults({
            design_name: '演示设计',
            overall_efficiency: 0.72,
            husking_rate: 0.78,
            breakage_rate: 0.08,
            pounding_force: 265,
            impact_energy: 12.5,
            overall_score: 0.68,
            grade: 'B级 - 良好设计',
            tolerance_report: {
                min_curvature: 0.008,
                overall_feasibility: 0.75,
                manufacturing_cost: 2100
            },
            design_feedback: ['曲率半径符合加工要求。', '运动平稳，冲击较小。', '压力角在合理范围内。', '脱壳率良好。', '破碎率在可接受范围。'],
            safety_warnings: ['升程偏小，舂捣效果可能不佳。']
        });
    }

    // ============ 初始化新功能事件绑定 ============
    function initNewFeatures() {
        initFeatureTabs();
        initDrawingCanvas();

        document.getElementById('btn-compare').addEventListener('click', handleCompareProfiles);
        document.getElementById('btn-cross-era').addEventListener('click', handleCrossEraComparison);
        document.getElementById('btn-analyze-vibration').addEventListener('click', handleVibrationAnalysis);
        document.getElementById('btn-test-design').addEventListener('click', handleTestDesign);
        document.getElementById('btn-clear-canvas').addEventListener('click', () => applyTemplate(currentTemplate));
    }

    if (typeof document !== 'undefined') {
        if (document.readyState === 'loading') {
            document.addEventListener('DOMContentLoaded', initNewFeatures);
        } else {
            setTimeout(initNewFeatures, 100);
        }
    }

})(typeof window !== 'undefined' ? window : this);
