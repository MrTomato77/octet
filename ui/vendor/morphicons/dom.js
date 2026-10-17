import { a as interpPolar, n as Spring, r as allocOutputs, t as SPRING_PRESETS } from "./spring-B_P92nf6.js";
import { i as serialize, n as iconToCubics, r as cubicsToPathD } from "./normalize-CYnN3Npw.js";
import { a as buildPlan, d as reversePts, n as resamplePath, o as centroid, t as resampleIcon } from "./resample-CXlaZ5ZQ.js";
import { a as paintOf, c as solid, o as painted, r as inside, s as signedArea } from "./region-BjECuWZR.js";
//#region src/dom/index.ts
const tickers = /* @__PURE__ */ new Set();
let rafId = 0;
let last = -1;
function loop(ts) {
	const dt = last < 0 ? 0 : Math.min(Math.max((ts - last) / 1e3, 0), .1);
	last = ts;
	for (const tick of [...tickers]) tick(dt);
	if (tickers.size > 0) rafId = requestAnimationFrame(loop);
	else {
		rafId = 0;
		last = -1;
	}
}
function addTicker(tick) {
	tickers.add(tick);
	if (rafId === 0) {
		last = -1;
		rafId = requestAnimationFrame(loop);
	}
}
function removeTicker(tick) {
	tickers.delete(tick);
	if (tickers.size === 0 && rafId !== 0) {
		cancelAnimationFrame(rafId);
		rafId = 0;
		last = -1;
	}
}
const samples = /* @__PURE__ */ new WeakMap();
const canon = /* @__PURE__ */ new WeakMap();
const plans = /* @__PURE__ */ new WeakMap();
function cubicsOf(icon) {
	if (typeof icon === "string" || paintOf(icon) === "stroke") return iconToCubics(icon);
	return solid(icon).flatMap((el) => {
		const ps = iconToCubics([el]);
		const odd = String(el[1]["fill-rule"]).trim() === "evenodd";
		let sum = 0;
		for (const p of ps) sum += signedArea(p.pts);
		for (const p of ps) {
			let flip = sum < 0;
			if (odd) {
				let pos = true;
				for (const o of ps) if (o !== p && inside(p.pts[0], p.pts[1], o.pts)) pos = !pos;
				flip = signedArea(p.pts) > 0 !== pos;
			}
			if (flip) p.pts = reversePts(p.pts);
		}
		return ps;
	});
}
function sampledOf(icon) {
	if (typeof icon === "string") return resampleIcon(icon);
	let s = samples.get(icon);
	if (!s) {
		s = cubicsOf(icon).map((p) => ({
			pts: resamplePath(p, 64),
			closed: p.closed
		}));
		samples.set(icon, s);
	}
	return s;
}
/** Canonical `d` of an icon: the input string verbatim, or the real cubics
*  quantized to 4 decimals (the at-rest snap; engine-stable bytes so SSR
*  hydration matches, see fmtCanon in core/serialize). Exported because it
*  is what a binding renders at SSR/rest before any runtime exists. A fill
*  icon's translucent duotone layer is left out (one path can't paint it). */
function canonicalD(icon) {
	if (typeof icon === "string") return icon;
	let d = canon.get(icon);
	if (!d) {
		d = cubicsToPathD(cubicsOf(icon));
		canon.set(icon, d);
	}
	return d;
}
function planBetween(src, dst) {
	if (typeof src === "string" || typeof dst === "string") return buildPlan(sampledOf(src), sampledOf(dst));
	let inner = plans.get(src);
	if (!inner) {
		inner = /* @__PURE__ */ new WeakMap();
		plans.set(src, inner);
	}
	let p = inner.get(dst);
	if (!p) {
		p = buildPlan(sampledOf(src), sampledOf(dst));
		inner.set(dst, p);
	}
	return p;
}
let regionMod = null;
let regionLoading = null;
let regionSince = 0;
let regionFailed = false;
let regionUrl;
let regionGen = 0;
let regionLoader = () => import("./region.js");
/** Replaces how the region normalizer is loaded (default: dynamic import of
*  the bundled chunk). E.g. `setRegionLoader(async () => region)` after a
*  static `import * as region from "morphicons/region"` to never touch the
*  network mid-interaction. The next need loads through the new loader. */
function setRegionLoader(loader) {
	regionLoader = loader;
	regionGen++;
	regionLoading = null;
	regionMod = null;
	regionFailed = false;
	regionUrl = void 0;
}
const reload = (url) => new Promise((ok, ko) => {
	const s = document.createElement("script");
	s.type = "module";
	s.onload = s.onerror = () => {
		s.remove();
		const m = globalThis[Symbol.for("morphicons.region")];
		m ? ok(m) : ko(new Error(url));
	};
	s.src = `${url}#${Date.now()}`;
	document.head.append(s);
});
const fromSource = () => {
	const spec = /import\(\s*["'`]([^"'`]+)/.exec(String(regionLoader))?.[1];
	const here = /\w+:\/\/[^\s)]+?(?=:\d+:\d+)/.exec(String((/* @__PURE__ */ new Error()).stack))?.[0];
	try {
		return spec && new URL(spec, here).href;
	} catch {}
};
/** Starts (or joins) loading the region normalizer. Call it early — on
*  idle, on hover — to keep the first stroke↔fill morph off the network. */
function preloadRegion() {
	if (regionMod) return Promise.resolve(regionMod);
	if (!regionLoading) {
		regionSince = Date.now();
		const url = regionUrl;
		const gen = regionGen;
		regionLoading = regionLoader().catch((e) => url ? reload(url) : Promise.reject(e)).then((m) => {
			if (gen === regionGen) regionMod = m;
			return m;
		}, (e) => {
			if (gen !== regionGen) throw e;
			regionLoading = null;
			regionFailed = true;
			regionUrl ??= /dynamically imported module:?\s*(\S+)/i.exec(String(e?.message))?.[1] ?? fromSource();
			throw e;
		});
	}
	return regionLoading;
}
const quietPreload = () => new Promise((ok, ko) => {
	if (typeof navigator === "undefined" ? false : navigator.connection?.saveData) return;
	const go = () => {
		regionFailed ? ko() : preloadRegion().then(ok, ko);
	};
	typeof requestIdleCallback === "function" ? requestIdleCallback(go, { timeout: 2e3 }) : setTimeout(go, 200);
});
/** How long a morph waits for the region chunk before flying the fallback
*  (RAIL's 100 ms response budget). A slow, failed or hanging load never
*  leaves the icon stuck on the old shape; the load goes on in the
*  background and the next morph flies. A load already older than the
*  budget isn't waited for again. */
const REGION_WAIT_MS = 100;
const paintAttrs = (p) => p === "stroke" ? {
	fill: "none",
	stroke: "inherit"
} : {
	fill: "currentColor",
	stroke: p === "fill" ? "none" : "inherit",
	"fill-rule": "nonzero"
};
const looks = /* @__PURE__ */ new WeakMap();
function restLook(icon) {
	if (typeof icon === "string" || paintOf(icon) === "stroke") return "stroke";
	let look = looks.get(icon);
	if (look) return look;
	let filled = 0;
	let enclosed = -1;
	look = "fill";
	for (const el of solid(icon)) {
		const s = painted(el[1].stroke);
		const f = painted(el[1].fill) || el[1].fill === void 0 && !s;
		if (s) look = "both";
		let a = 0;
		for (const p of iconToCubics([el])) a += Math.abs(signedArea(p.pts));
		if (f) filled += a;
		else if (s) enclosed = Math.max(enclosed, 0) + a;
	}
	if (enclosed > 1.5 * filled) look = "stroke";
	looks.set(icon, look);
	return look;
}
/** The paint attributes a binding renders on the `<path>` with its first
*  icon (SSR included), so a fill icon is filled before any script runs.
*  None for a stroke icon: it inherits the `<svg>`'s stroke as always. */
function restAttrs(icon) {
	if (icon === void 0 || paintOf(icon) === "stroke") return void 0;
	return paintAttrs(restLook(icon));
}
function popPlan(from, to) {
	const n = 64;
	const item = ({ pts, closed }, half) => {
		if (pts.length > 128) pts = pts.filter((_, i) => i % 4 < 2);
		const c = centroid(pts);
		const p = pts.map((v, i) => v - c[i & 1]);
		const z = /* @__PURE__ */ new Float64Array(128);
		return {
			aC: half ? z : p,
			bT: half ? p : z,
			ca: c,
			cb: c,
			theta: 0,
			lnSigma: 0,
			closed,
			half,
			block: null
		};
	};
	return {
		n,
		items: [...from.map((s) => item(s, 0)), ...to.map((s) => item(s, 1))]
	};
}
function resolveSpring(s) {
	if (typeof s === "string") return SPRING_PRESETS[s];
	const d = SPRING_PRESETS.snappy;
	return {
		k: s?.stiffness ?? d.k,
		c: s?.damping ?? d.c
	};
}
/** Creates the morph instance over a `<path>` and paints the initial icon. */
function createMorph(el, icon, options) {
	const spring = new Spring();
	let reducedMotion = options?.reducedMotion ?? "never";
	let strokeWidth = options?.strokeWidth ?? 2;
	let target = icon;
	let rest = true;
	let plan = null;
	let out = null;
	let closed = null;
	let t = 1;
	let flying = false;
	let dead = false;
	let regionFlight = false;
	let drawn = false;
	let pending = null;
	let paint = "stroke";
	let split = 0;
	let popFrom = "stroke";
	let popBase = null;
	const paintAs = (p) => {
		if (p === paint) return;
		for (const [k, v] of Object.entries(paintAttrs(p))) el.setAttribute(k, v);
		paint = p;
	};
	const paintRest = (ic) => {
		const r = regionMod && paintOf(ic) === "fill" ? regionMod.regionOf(ic, strokeWidth) : ic;
		el.setAttribute("d", canonicalD(r));
		paintAs(restLook(r));
	};
	const upgrade = (load) => {
		const icon = target;
		load.then(() => {
			if (!dead && rest && target === icon && pending === null) paintRest(icon);
		}, () => {});
	};
	paintRest(icon);
	if (paintOf(icon) === "fill" && !regionMod) upgrade(quietPreload());
	const shown = (tt) => !split ? [0, plan?.items.length ?? 0] : tt < .5 ? [0, split] : [split, plan?.items.length ?? 0];
	const render = (tt) => {
		const p = plan;
		const o = out;
		const cl = closed;
		if (!p || !o || !cl) return;
		t = tt;
		interpPolar(p, tt, o);
		drawn = true;
		const [lo, hi] = shown(tt);
		if (split) paintAs(tt < .5 ? popFrom : restLook(target));
		el.setAttribute("d", serialize(o.slice(lo, hi), cl.slice(lo, hi)));
	};
	const stop = () => {
		if (!flying) return;
		flying = false;
		removeTicker(tick);
	};
	const tick = (dt) => {
		const settled = spring.step(dt);
		render(spring.x);
		if (settled) {
			stop();
			settle();
		}
	};
	const settle = () => {
		rest = true;
		plan = null;
		out = null;
		closed = null;
		t = 1;
		spring.x = 1;
		spring.v = 0;
		regionFlight = false;
		split = 0;
		paintRest(target);
		if (paintOf(target) === "fill" && !regionMod && (regionLoading || !regionFailed)) upgrade(preloadRegion());
	};
	/** The current shape as plan source: the at-rest icon, or the rendered
	*  buffers (already N points per subpath). */
	const snapshot = () => {
		const p = plan;
		const o = out;
		if (rest || !p || !o) return sampledOf(target);
		const [lo, hi] = shown(t);
		return o.slice(lo, hi).map((buf, k) => ({
			pts: Float64Array.from(buf),
			closed: p.items[lo + k].closed,
			hole: p.items[lo + k].hole
		}));
	};
	/** A morph from the current shape to `icon` needs regions when either
	*  side is filled, or when the shape in flight already is a region. */
	const needsRegion = (icon) => !rest && regionFlight || paintOf(icon) === "fill" || paintOf(target) === "fill";
	const retarget = (icon, pop) => {
		if (!rest && plan && out && !drawn) {
			t = 0;
			interpPolar(plan, 0, out);
		}
		if (pop || !rest && split) {
			const from = snapshot();
			popBase = rest ? target : null;
			popFrom = paint;
			plan = popPlan(from, sampledOf(icon));
			split = from.length;
			regionFlight = false;
		} else if (needsRegion(icon)) {
			const mod = regionMod;
			plan = rest || !out ? mod.regionPlan(target, icon, strokeWidth) : mod.regionPlanFrom(regionFlight ? snapshot() : serialize(out, closed ?? void 0), icon, strokeWidth);
			regionFlight = true;
			split = 0;
			paintAs("fill");
		} else {
			plan = rest ? planBetween(target, icon) : buildPlan(snapshot(), sampledOf(icon));
			regionFlight = false;
		}
		out = allocOutputs(plan);
		drawn = false;
		closed = plan.items.map((it) => it.closed);
		target = icon;
		rest = false;
	};
	const whenRegion = (icon, run, fallback) => {
		if (regionMod || !rest && split || !needsRegion(icon)) return false;
		const req = {};
		pending = req;
		const give = () => {
			if (dead || pending !== req) return;
			pending = null;
			fallback();
		};
		const load = preloadRegion();
		if (Date.now() - regionSince > REGION_WAIT_MS) {
			give();
			return true;
		}
		const timer = setTimeout(give, REGION_WAIT_MS - (Date.now() - regionSince));
		load.then(() => {
			clearTimeout(timer);
			if (dead || pending !== req) return;
			pending = null;
			run();
		}, () => {
			clearTimeout(timer);
			give();
		});
		return true;
	};
	const setNow = (icon) => {
		stop();
		pending = null;
		target = icon;
		settle();
	};
	/** True when the policy says this morphTo must jump instead of flying. */
	const motionOff = () => {
		if (reducedMotion === "always") return true;
		if (reducedMotion !== "user") return false;
		if (typeof matchMedia === "undefined") return false;
		return matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;
	};
	const seek = (icon, tt, pop = false) => {
		if (dead) return;
		if (!pop && whenRegion(icon, () => seek(icon, tt), () => seek(icon, tt, true))) return;
		pending = null;
		let reuse = !rest && plan !== null && icon === target;
		if (reuse && split && regionMod && popBase !== null) {
			target = popBase;
			rest = true;
			reuse = false;
		}
		stop();
		spring.v = 0;
		if (!reuse) retarget(icon, pop);
		render(tt);
	};
	const fly = (icon, sp, pop) => {
		if (dead) return;
		if (icon === target && (rest || flying)) {
			pending = null;
			return;
		}
		if (motionOff()) {
			setNow(icon);
			return;
		}
		if (!pop && whenRegion(icon, () => fly(icon, sp, false), () => fly(icon, sp, true))) return;
		pending = null;
		const { k, c } = resolveSpring(sp);
		spring.config(k, c);
		retarget(icon, pop);
		spring.start();
		if (!flying) {
			flying = true;
			addTicker(tick);
		}
	};
	return {
		morphTo: (icon, sp) => fly(icon, sp, false),
		set(icon) {
			if (dead) return;
			setNow(icon);
		},
		seek,
		get progress() {
			return rest ? 1 : t;
		},
		set progress(v) {
			if (!dead) seek(target, v);
		},
		get reducedMotion() {
			return reducedMotion;
		},
		set reducedMotion(v) {
			reducedMotion = v;
		},
		get strokeWidth() {
			return strokeWidth;
		},
		set strokeWidth(v) {
			strokeWidth = v;
		},
		destroy() {
			stop();
			dead = true;
			pending = null;
			plan = null;
			out = null;
			closed = null;
		}
	};
}
//#endregion
export { canonicalD, createMorph, preloadRegion, restAttrs, setRegionLoader };
