//#region src/core/interpolate.ts
/** Preallocated output buffers for a plan (zero allocation per frame). */
function allocOutputs(plan) {
	return plan.items.map(() => new Float64Array(2 * plan.n));
}
function interpPolar(plan, t, out) {
	for (let k = 0; k < plan.items.length; k++) {
		const it = plan.items[k];
		const o = out[k];
		const n = plan.n;
		const s = Math.exp(it.lnSigma * t);
		const ang = it.theta * t;
		const cos = Math.cos(ang) * s;
		const sin = Math.sin(ang) * s;
		let cx;
		let cy;
		if (it.block) {
			const [ox, oy] = it.block.off;
			const [dx, dy] = it.block.drift;
			cx = it.ca[0] + dx * t + (ox * cos - oy * sin - ox);
			cy = it.ca[1] + dy * t + (ox * sin + oy * cos - oy);
		} else {
			cx = it.ca[0] + (it.cb[0] - it.ca[0]) * t;
			cy = it.ca[1] + (it.cb[1] - it.ca[1]) * t;
		}
		const m = it.half === void 0 ? t : Math.min(1, Math.max(0, 2 * t - it.half));
		for (let i = 0; i < n; i++) {
			const px = it.aC[2 * i] + (it.bT[2 * i] - it.aC[2 * i]) * m;
			const py = it.aC[2 * i + 1] + (it.bT[2 * i + 1] - it.aC[2 * i + 1]) * m;
			o[2 * i] = cx + px * cos - py * sin;
			o[2 * i + 1] = cy + px * sin + py * cos;
		}
	}
}
/** Raw coordinate lerp (same correspondence, no decomposition). */
function interpLinear(plan, t, out) {
	for (let k = 0; k < plan.items.length; k++) {
		const it = plan.items[k];
		const o = out[k];
		const n = plan.n;
		for (let i = 0; i < n; i++) {
			o[2 * i] = it.a[2 * i] + (it.bO[2 * i] - it.a[2 * i]) * t;
			o[2 * i + 1] = it.a[2 * i + 1] + (it.bO[2 * i + 1] - it.a[2 * i + 1]) * t;
		}
	}
}
//#endregion
//#region src/core/spring.ts
var Spring = class {
	x = 1;
	v = 0;
	k = 250;
	c = 24;
	config(k, c) {
		this.k = k;
		this.c = c;
	}
	/** Starts (or restarts mid-flight) preserving velocity. */
	start() {
		this.x = 0;
		if (this.v > 14) this.v = 14;
		if (this.v < -14) this.v = -14;
	}
	/** Advances dt seconds. Returns true on settle (|1−x| < 0.001 ∧ |v| < 0.02). */
	step(dt) {
		const steps = Math.max(1, Math.min(16, Math.ceil(dt / (1 / 240))));
		const s = dt / steps;
		for (let i = 0; i < steps; i++) {
			const a = this.k * (1 - this.x) - this.c * this.v;
			this.v += a * s;
			this.x += this.v * s;
		}
		return Math.abs(1 - this.x) < .001 && Math.abs(this.v) < .02;
	}
};
/** Spring presets (ζ = c/(2√k)) with the API's public names. */
const SPRING_PRESETS = {
	/** ζ = 1.00 — critically damped, no overshoot. */
	smooth: {
		k: 170,
		c: 26
	},
	/** ζ = 0.73 — fast, subtle overshoot. */
	snappy: {
		k: 420,
		c: 30
	},
	/** ζ = 0.40 — playful. */
	bouncy: {
		k: 300,
		c: 14
	}
};
//#endregion
export { interpPolar as a, interpLinear as i, Spring as n, allocOutputs as r, SPRING_PRESETS as t };
