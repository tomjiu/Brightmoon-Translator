import { useEffect, useRef } from 'react';

/**
 * Dev 月球主题的「边框高斯光池」— v6 WebGL Shader。
 *
 * GPU Fragment Shader 一次性计算整个边框光效，替代 CPU 逐点 stroke。
 * - CPU：鼠标 → O(1) 解析法找最近边框点 → 收敛插值 → 传 uniform
 * - GPU：每像素算圆角矩形 SDF + 高斯 → 变化带宽 + 变化 alpha
 *
 * 与原版 canvas 逐点 stroke 视觉一致：
 * - 带宽随高斯变化：BW(1.1px) → W0(3.4px)
 * - alpha 随高斯变化：ALO(0.13) → APK(0.78)
 * - 圆角处光带贴合弧线（SDF 天然处理）
 *
 * 仅主窗口 dev / dev-light 主题下挂载。
 */

const VERT_SRC = `
attribute vec2 a_position;
void main() {
  gl_Position = vec4(a_position, 0.0, 1.0);
}
`;

const FRAG_SRC = `
precision highp float;
uniform vec2  u_resolution;
uniform float u_dpr;
uniform vec2  u_lightPos;
uniform float u_margin;
uniform float u_radius;
uniform float u_sigma;
uniform float u_amp;
uniform float u_base;
uniform float u_peakAlpha;
uniform float u_baseAlpha;
uniform float u_bw;
uniform float u_w0;
uniform vec3  u_color;

// 标准圆角矩形 SDF（iq）
float sdRoundBox(vec2 p, vec2 b, float r) {
  vec2 q = abs(p) - b + r;
  return min(max(q.x, q.y), 0.0) + length(max(q, vec2(0.0))) - r;
}

void main() {
  vec2 p = vec2(gl_FragCoord.x, u_resolution.y * u_dpr - gl_FragCoord.y) / u_dpr;

  vec2 center = u_resolution * 0.5;
  vec2 halfSize = center - vec2(u_margin);

  float sdf = sdRoundBox(p - center, halfSize, u_radius);
  float distToBorder = abs(sdf);

  // 高斯亮度（与原版公式一致）
  float distToMouse = distance(p, u_lightPos);
  float amp = u_base + u_amp * exp(-(distToMouse * distToMouse) / (2.0 * u_sigma * u_sigma));
  float k = pow(amp / (u_base + u_amp), 0.9);

  // 变化带宽：BW(1.1) → W0(3.4)，与原版线宽变化一致
  float bandHalf = u_bw + k * (u_w0 - u_bw);

  // 边框带 mask：0.5px 软边缘
  float borderMask = 1.0 - smoothstep(bandHalf - 0.5, bandHalf, distToBorder);
  if (borderMask < 0.001) {
    gl_FragColor = vec4(0.0);
    return;
  }

  // alpha：ALO(0.13) → APK(0.78)
  float alpha = u_baseAlpha + (u_peakAlpha - u_baseAlpha) * k;
  alpha *= borderMask;

  gl_FragColor = vec4(u_color * alpha, alpha);
}
`;

export default function DevComet() {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const gl = canvas.getContext('webgl', {
      alpha: true,
      premultipliedAlpha: true,
      antialias: false,
      depth: false,
      stencil: false,
    });
    if (!gl) return;

    const root = document.documentElement;

    const compile = (type: number, src: string): WebGLShader => {
      const sh = gl.createShader(type)!;
      gl.shaderSource(sh, src);
      gl.compileShader(sh);
      if (!gl.getShaderParameter(sh, gl.COMPILE_STATUS)) {
        const log = gl.getShaderInfoLog(sh);
        gl.deleteShader(sh);
        throw new Error('Shader compile error: ' + log);
      }
      return sh;
    };

    const vs = compile(gl.VERTEX_SHADER, VERT_SRC);
    const fs = compile(gl.FRAGMENT_SHADER, FRAG_SRC);
    const prog = gl.createProgram()!;
    gl.attachShader(prog, vs);
    gl.attachShader(prog, fs);
    gl.linkProgram(prog);
    if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) {
      throw new Error('Program link error: ' + gl.getProgramInfoLog(prog));
    }
    gl.useProgram(prog);

    const buf = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, buf);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([
      -1, -1,  1, -1, -1,  1,
      -1,  1,  1, -1,  1,  1,
    ]), gl.STATIC_DRAW);
    const aPos = gl.getAttribLocation(prog, 'a_position');
    gl.enableVertexAttribArray(aPos);
    gl.vertexAttribPointer(aPos, 2, gl.FLOAT, false, 0, 0);

    const U = {
      resolution: gl.getUniformLocation(prog, 'u_resolution')!,
      dpr: gl.getUniformLocation(prog, 'u_dpr')!,
      lightPos: gl.getUniformLocation(prog, 'u_lightPos')!,
      margin: gl.getUniformLocation(prog, 'u_margin')!,
      radius: gl.getUniformLocation(prog, 'u_radius')!,
      sigma: gl.getUniformLocation(prog, 'u_sigma')!,
      amp: gl.getUniformLocation(prog, 'u_amp')!,
      base: gl.getUniformLocation(prog, 'u_base')!,
      peakAlpha: gl.getUniformLocation(prog, 'u_peakAlpha')!,
      baseAlpha: gl.getUniformLocation(prog, 'u_baseAlpha')!,
      bw: gl.getUniformLocation(prog, 'u_bw')!,
      w0: gl.getUniformLocation(prog, 'u_w0')!,
      color: gl.getUniformLocation(prog, 'u_color')!,
    };

    gl.enable(gl.BLEND);
    gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);

    let W = 0, H = 0, dpr = 1;
    let margin = 4;
    let radius = 0;
    let lightX = 0, lightY = 0;
    let targetX = 0, targetY = 0;
    let rafId = 0;
    let animating = false;

    // 光效参数（与原版 canvas 逐点 stroke 完全一致）
    const SIG = 26;
    const AMP = 0.4;
    const BASE = 0.1;
    const PEAK_ALPHA = 0.78;
    const BASE_ALPHA = 0.13;
    const BW = 1.1;   // 基线线宽
    const W0 = 3.4;   // 峰值线宽

    const clampN = (v: number, lo: number, hi: number) => Math.max(lo, Math.min(hi, v));

    const projectToArc = (mx: number, my: number, cx: number, cy: number, r: number): [number, number] => {
      const dx = mx - cx, dy = my - cy;
      const len = Math.hypot(dx, dy);
      if (len < 0.001) return [cx + r, cy];
      return [cx + (dx / len) * r, cy + (dy / len) * r];
    };

    const nearestPointOnBorder = (
      mx: number, my: number,
      x0: number, y0: number, x1: number, y1: number, cr: number
    ): [number, number] => {
      const inL = mx < x0 + cr;
      const inR = mx > x1 - cr;
      const inT = my < y0 + cr;
      const inB = my > y1 - cr;

      if (inL && inT) return projectToArc(mx, my, x0 + cr, y0 + cr, cr);
      if (inR && inT) return projectToArc(mx, my, x1 - cr, y0 + cr, cr);
      if (inR && inB) return projectToArc(mx, my, x1 - cr, y1 - cr, cr);
      if (inL && inB) return projectToArc(mx, my, x0 + cr, y1 - cr, cr);

      const dT = Math.abs(my - y0);
      const dR = Math.abs(mx - x1);
      const dB = Math.abs(my - y1);
      const dL = Math.abs(mx - x0);
      const minD = Math.min(dT, dR, dB, dL);

      if (minD === dT) return [clampN(mx, x0 + cr, x1 - cr), y0];
      if (minD === dR) return [x1, clampN(my, y0 + cr, y1 - cr)];
      if (minD === dB) return [clampN(mx, x0 + cr, x1 - cr), y1];
      return [x0, clampN(my, y0 + cr, y1 - cr)];
    };

    const render = () => {
      gl.clearColor(0, 0, 0, 0);
      gl.clear(gl.COLOR_BUFFER_BIT);

      gl.uniform2f(U.resolution, W, H);
      gl.uniform1f(U.dpr, dpr);
      gl.uniform2f(U.lightPos, lightX, lightY);
      gl.uniform1f(U.margin, margin);
      gl.uniform1f(U.radius, radius);
      gl.uniform1f(U.sigma, SIG);
      gl.uniform1f(U.amp, AMP);
      gl.uniform1f(U.base, BASE);
      gl.uniform1f(U.peakAlpha, PEAK_ALPHA);
      gl.uniform1f(U.baseAlpha, BASE_ALPHA);
      gl.uniform1f(U.bw, BW);
      gl.uniform1f(U.w0, W0);

      const isLight = root.classList.contains('light');
      gl.uniform3f(U.color,
        isLight ? 28 / 255 : 1.0,
        isLight ? 32 / 255 : 1.0,
        isLight ? 40 / 255 : 1.0
      );

      gl.drawArrays(gl.TRIANGLES, 0, 6);
    };

    const step = () => {
      const dx = targetX - lightX;
      const dy = targetY - lightY;
      const d = Math.hypot(dx, dy);
      if (d < 0.01) {
        lightX = targetX;
        lightY = targetY;
      } else {
        lightX += dx * 0.16;
        lightY += dy * 0.16;
      }

      const lx = (lightX / W) * 100 || 50;
      const ly = (lightY / H) * 100 || 46;
      const la = 90 + (Math.atan2(46 - ly, 50 - lx) * 180) / Math.PI;
      const sx = (50 - lx) * 0.2;
      const sy = (46 - ly) * 0.18;
      root.style.setProperty('--lx', `${lx.toFixed(2)}%`);
      root.style.setProperty('--ly', `${ly.toFixed(2)}%`);
      root.style.setProperty('--la', `${la.toFixed(1)}deg`);
      root.style.setProperty('--sx', `${sx.toFixed(2)}px`);
      root.style.setProperty('--sy', `${sy.toFixed(2)}px`);
      root.style.setProperty('--ix', `${(-sx * 1.6).toFixed(2)}px`);
      root.style.setProperty('--iy', `${(-sy * 1.6).toFixed(2)}px`);

      render();

      if (lightX === targetX && lightY === targetY && animating) {
        animating = false;
        return;
      }
      rafId = requestAnimationFrame(step);
    };

    const wake = () => {
      if (!W || !H || animating) return;
      animating = true;
      cancelAnimationFrame(rafId);
      rafId = requestAnimationFrame(step);
    };

    const resize = () => {
      W = window.innerWidth;
      H = window.innerHeight;
      dpr = window.devicePixelRatio || 1;
      canvas.width = W * dpr;
      canvas.height = H * dpr;
      canvas.style.width = W + 'px';
      canvas.style.height = H + 'px';
      gl.viewport(0, 0, canvas.width, canvas.height);

      margin = 4;
      radius = Math.max(6, Math.min(30, Math.min(W, H) * 0.06));

      const [nx, ny] = nearestPointOnBorder(W / 2, H / 2, margin, margin, W - margin, H - margin, radius);
      lightX = nx;
      lightY = ny;
      targetX = nx;
      targetY = ny;
      wake();
    };

    const onMove = (e: MouseEvent) => {
      if (!W || !H) return;
      const [nx, ny] = nearestPointOnBorder(e.clientX, e.clientY, margin, margin, W - margin, H - margin, radius);
      targetX = nx;
      targetY = ny;
      wake();
    };

    resize();
    window.addEventListener('resize', resize);
    window.addEventListener('mousemove', onMove);

    return () => {
      cancelAnimationFrame(rafId);
      window.removeEventListener('resize', resize);
      window.removeEventListener('mousemove', onMove);
    };
  }, []);

  return <canvas ref={canvasRef} className="pointer-events-none fixed inset-0 z-[9999]" />;
}
