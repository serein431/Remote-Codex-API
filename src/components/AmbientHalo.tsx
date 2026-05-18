import { useEffect, useRef } from "react";
import * as THREE from "three";

type Theme = "dark" | "light";

type AmbientHaloProps = {
  theme: Theme;
};

function getPalette(theme: Theme) {
  if (theme === "light") {
    return {
      core: "#050608",
      fringe: "#8ba3cc",
      isLightMode: 1,
    };
  }

  return {
    core: "#ffffff",
    fringe: "#4a88ff",
    isLightMode: 0,
  };
}

const vertexShader = `
  varying vec2 vUv;

  void main() {
    vUv = uv;
    gl_Position = vec4(position, 1.0);
  }
`;

const fragmentShader = `
  uniform float u_time;
  uniform vec2 u_resolution;
  uniform vec2 u_mouse;
  uniform vec3 u_colorCore;
  uniform vec3 u_colorFringe;
  uniform float u_isLightMode;
  varying vec2 vUv;

  vec2 hash(vec2 p) {
    p = vec2(dot(p, vec2(127.1, 311.7)), dot(p, vec2(269.5, 183.3)));
    return -1.0 + 2.0 * fract(sin(p) * 43758.5453123);
  }

  float noise(vec2 p) {
    const float K1 = 0.366025404;
    const float K2 = 0.211324865;
    vec2 i = floor(p + (p.x + p.y) * K1);
    vec2 a = p - i + (i.x + i.y) * K2;
    vec2 o = (a.x > a.y) ? vec2(1.0, 0.0) : vec2(0.0, 1.0);
    vec2 b = a - o + K2;
    vec2 c = a - 1.0 + 2.0 * K2;
    vec3 h = max(0.5 - vec3(dot(a, a), dot(b, b), dot(c, c)), 0.0);
    vec3 n = h * h * h * h * vec3(dot(a, hash(i + 0.0)), dot(b, hash(i + o)), dot(c, hash(i + 1.0)));
    return dot(n, vec3(70.0));
  }

  float sdArc(vec2 p, vec2 center, float radius, float width, float warp) {
    p.y += sin(p.x * 3.0 + u_time * 0.5) * warp;
    p.x += noise(p * 2.0 + u_time * 0.2) * (warp * 0.5);
    float d = length(p - center) - radius;
    return abs(d) - width;
  }

  void main() {
    vec2 uv = gl_FragCoord.xy / u_resolution.xy;
    vec2 st = uv;
    st.x *= u_resolution.x / u_resolution.y;

    vec2 mouseOffset = (u_mouse - 0.5) * 0.1;
    st += mouseOffset;

    vec2 center = vec2(0.2, 0.5);

    float d1 = sdArc(st, center, 0.8, 0.01, 0.1);
    float d2 = sdArc(st, center, 0.82, 0.04, 0.15);

    float coreGlow = exp(-d1 * 40.0);
    float fringeGlow = exp(-d2 * 15.0);
    float wash = smoothstep(1.0, -0.2, st.x) * 0.3;

    vec3 finalColor = vec3(0.0);
    finalColor += u_colorCore * coreGlow;
    finalColor += u_colorFringe * fringeGlow;
    finalColor += u_colorFringe * wash * (sin(u_time) * 0.1 + 0.9);

    float alpha = clamp(coreGlow + fringeGlow + wash, 0.0, 1.0);
    finalColor = vec3(1.0) - exp(-finalColor * 2.0);

    if (u_isLightMode > 0.5) {
      alpha = clamp((coreGlow * 1.5 + fringeGlow + wash * 0.5), 0.0, 0.6);
    }

    gl_FragColor = vec4(finalColor, alpha);
  }
`;

export function AmbientHalo({ theme }: AmbientHaloProps) {
  const containerRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const palette = getPalette(theme);
    const scene = new THREE.Scene();
    const camera = new THREE.OrthographicCamera(-1, 1, 1, -1, 0.1, 10);
    camera.position.z = 1;

    const renderer = new THREE.WebGLRenderer({
      alpha: true,
      antialias: true,
      powerPreference: "high-performance",
    });
    renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
    container.appendChild(renderer.domElement);

    const geometry = new THREE.PlaneGeometry(2, 2);
    const material = new THREE.ShaderMaterial({
      vertexShader,
      fragmentShader,
      uniforms: {
        u_time: { value: 0 },
        u_resolution: { value: new THREE.Vector2(window.innerWidth, window.innerHeight) },
        u_mouse: { value: new THREE.Vector2(0.5, 0.5) },
        u_colorCore: { value: new THREE.Color(palette.core) },
        u_colorFringe: { value: new THREE.Color(palette.fringe) },
        u_isLightMode: { value: palette.isLightMode },
      },
      transparent: true,
      blending: THREE.NormalBlending,
    });

    const mesh = new THREE.Mesh(geometry, material);
    scene.add(mesh);

    const targetMouse = new THREE.Vector2(0.5, 0.5);
    const currentMouse = new THREE.Vector2(0.5, 0.5);
    const motionQuery = window.matchMedia("(prefers-reduced-motion: reduce)");
    let reduceMotion = motionQuery.matches;
    let frameId = 0;
    let frameCount = 0;

    const resize = () => {
      const width = window.innerWidth;
      const height = window.innerHeight;
      renderer.setSize(width, height);
      material.uniforms.u_resolution.value.set(width, height);
    };

    const onPointerMove = (event: PointerEvent) => {
      targetMouse.x = event.clientX / window.innerWidth;
      targetMouse.y = 1 - event.clientY / window.innerHeight;
    };

    const onMotionChange = (event: MediaQueryListEvent) => {
      reduceMotion = event.matches;
    };

    const clock = new THREE.Clock();
    const animate = () => {
      frameId = window.requestAnimationFrame(animate);
      material.uniforms.u_time.value = reduceMotion ? 0 : clock.getElapsedTime();
      currentMouse.lerp(targetMouse, reduceMotion ? 0.1 : 0.05);
      material.uniforms.u_mouse.value.copy(currentMouse);
      frameCount += 1;
      container.dataset.auraFrame = String(frameCount);
      renderer.render(scene, camera);
    };

    resize();
    window.addEventListener("resize", resize);
    window.addEventListener("pointermove", onPointerMove);
    motionQuery.addEventListener("change", onMotionChange);
    animate();

    return () => {
      window.cancelAnimationFrame(frameId);
      window.removeEventListener("resize", resize);
      window.removeEventListener("pointermove", onPointerMove);
      motionQuery.removeEventListener("change", onMotionChange);
      geometry.dispose();
      material.dispose();
      renderer.dispose();
      renderer.forceContextLoss();
      mesh.removeFromParent();
      container.innerHTML = "";
    };
  }, [theme]);

  return <div ref={containerRef} className="ambient-halo" aria-hidden="true" />;
}
