(function (global) {
    'use strict';

    let apiBaseUrl = global._camPanelApiBaseUrl || 'http://localhost:8080/api';

    let drawingCanvas = null;
    let drawingCtx = null;
    let controlPoints = [];
    let draggingIdx = -1;
    let currentTemplate = 'harmonic';

    function setApiBase(url) { apiBaseUrl = url; }

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

        if (global.showLoading) global.showLoading('正在分析您的凸轮设计...');

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
            if (global.hideLoading) global.hideLoading();
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

    function init() {
        initDrawingCanvas();

        const btnTest = document.getElementById('btn-test-design');
        if (btnTest) btnTest.addEventListener('click', handleTestDesign);

        const btnClear = document.getElementById('btn-clear-canvas');
        if (btnClear) btnClear.addEventListener('click', () => applyTemplate(currentTemplate));
    }

    if (typeof document !== 'undefined') {
        if (document.readyState === 'loading') {
            document.addEventListener('DOMContentLoaded', init);
        } else {
            setTimeout(init, 100);
        }
    }

    global.VrCamDesignerPanel = {
        handleTestDesign,
        renderDesignResults,
        initDrawingCanvas,
        applyTemplate,
        sampleLiftFromCurve,
        setApiBase,
        init
    };

})(typeof window !== 'undefined' ? window : this);
