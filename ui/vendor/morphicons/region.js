import { n as iconToCubics } from "./normalize-CYnN3Npw.js";
import { c as makeItem, d as reversePts, f as rotatePts, i as applyGlobal, l as pairUp, n as resamplePath, o as centroid, s as costMatrix, u as procrustes } from "./resample-CXlaZ5ZQ.js";
import { a as paintOf, i as isRegion, l as translucent, n as distSeg, r as inside, s as signedArea, t as REGION_ATTR } from "./region-BjECuWZR.js";
//#region src/region/plan.ts
const REGION_THRESHOLDS = [
	Math.PI / 4,
	Math.PI / 3,
	Math.PI / 2,
	Math.PI
];
const FINE = .3;
function coarse(path, s) {
	const n = s.length / 2;
	let j = 0;
	for (let v = 0; v < path.length; v += 6) for (let k = 0;; k++) {
		if (k === n) return true;
		const a = (j + k) % n;
		const b = (a + 1) % n;
		if (distSeg(path[v], path[v + 1], s[2 * a], s[2 * a + 1], s[2 * b], s[2 * b + 1]) <= FINE) {
			j = a;
			break;
		}
	}
	return false;
}
/** Samples a normalized region: one Sampled per contour, every one closed,
*  `hole` read from the winding (outers positive, holes negative). N per
*  contour: 64, or 128 for every contour of an icon that 64 can't follow
*  (snowflake tips, spiral turns: the landing would snap). */
function resampleRegion(icon, N) {
	const paths = iconToCubics(icon);
	const at = (n) => paths.map((path) => {
		let pts;
		for (const th of REGION_THRESHOLDS) try {
			pts = resamplePath({
				pts: path.pts,
				closed: true
			}, n, th);
			break;
		} catch {}
		const s = pts;
		return {
			pts: s,
			closed: true,
			hole: signedArea(s) < 0
		};
	});
	const s = at(N ?? 64);
	return N || !paths.some((p, k) => coarse(p.pts, s[k].pts)) ? s : at(128);
}
const REGION_LAMBDA = .3;
const FOLD_SLACK = .1;
const FOLD_TRIES = 16;
function folds(a, b) {
	const L = a.length;
	const p = new Float64Array(L);
	for (let i = 0; i < L; i++) p[i] = (a[i] + b[i]) / 2;
	for (let i = 0; i < L; i += 2) {
		const ax = p[i];
		const ay = p[i + 1];
		const ux = p[(i + 2) % L] - ax;
		const uy = p[(i + 3) % L] - ay;
		for (let j = i + 4; j < L - (i ? 0 : 2); j += 2) {
			const cx = p[j];
			const cy = p[j + 1];
			const vx = p[(j + 2) % L] - cx;
			const vy = p[(j + 3) % L] - cy;
			if ((ux * (cy - ay) - uy * (cx - ax)) * (ux * (cy + vy - ay) - uy * (cx + vx - ax)) >= 0) continue;
			if ((vx * (ay - cy) - vy * (ax - cx)) * (vx * (ay + uy - cy) - vy * (ax + ux - cx)) < 0) return true;
		}
	}
	return false;
}
function outerItem(a, b) {
	const n = a.length / 2;
	const k = n > 64 ? 2 : 1;
	const half = (p, o) => k > 1 ? p.filter((_, i) => (i >> 1) % 2 === o) : p;
	const as = half(a, 0);
	const ca = centroid(as);
	const bs = [half(b, 0), half(b, 1)];
	const cbs = bs.map(centroid);
	const offs = [];
	for (let off = 0; off < n; off++) {
		const j = off % k;
		const s = procrustes(as, rotatePts(bs[j], (off - j) / k), ca, cbs[j]);
		offs.push([off, s.res + REGION_LAMBDA * Math.abs(s.theta) / Math.PI]);
	}
	offs.sort((x, y) => x[1] - y[1]);
	let best;
	for (const [off, score] of offs.slice(0, FOLD_TRIES)) {
		if (score > offs[0][1] + FOLD_SLACK) break;
		const it = makeItem(a, rotatePts(b, off), false, false, false, REGION_LAMBDA);
		it.closed = true;
		best ??= it;
		if (!folds(half(it.aC, 0), half(it.bT, 0))) return it;
	}
	return best;
}
function parentOf(hole, subs) {
	const n = hole.length / 2;
	let best = -1;
	let bestArea = Infinity;
	subs.forEach((s, k) => {
		if (s.hole) return;
		let votes = 0;
		for (const v of [
			0,
			Math.floor(n / 3),
			Math.floor(2 * n / 3)
		]) if (inside(hole[2 * v], hole[2 * v + 1], s.pts)) votes++;
		if (votes < 2) return;
		const ar = Math.abs(signedArea(s.pts));
		if (ar < bestArea) {
			bestArea = ar;
			best = k;
		}
	});
	return best;
}
function assign(C, skip, one, cover) {
	const L = C.length;
	const S = one.length;
	const f = new Array(L).fill(-1);
	const mult = new Array(S).fill(0);
	if ((S + 1) ** L > 1e5) {
		const pairs = [];
		for (let i = 0; i < L; i++) for (let j = 0; j < S; j++) if (C[i][j] < Infinity) pairs.push([
			C[i][j],
			i,
			j
		]);
		pairs.sort((x, y) => x[0] - y[0]);
		for (const [, i, j] of pairs) if (f[i] < 0 && !mult[j]) {
			f[i] = j;
			mult[j] = 1;
		}
		for (let i = 0; i < L; i++) {
			if (f[i] >= 0) continue;
			for (let j = 0; j < S; j++) if (!one[j] && (f[i] < 0 || C[i][j] < C[i][f[i]])) f[i] = j;
		}
		return f;
	}
	let best = f.slice();
	let bc = Infinity;
	const rec = (i, acc, covered) => {
		if (acc >= bc || cover && S - covered > L - i) return;
		if (i === L) {
			bc = acc;
			best = f.slice();
			return;
		}
		for (let j = 0; j < S; j++) {
			if (one[j] && mult[j] || !(C[i][j] < Infinity)) continue;
			f[i] = j;
			mult[j]++;
			rec(i + 1, acc + C[i][j], covered + (mult[j] === 1 ? 1 : 0));
			mult[j]--;
		}
		f[i] = -1;
		rec(i + 1, acc + skip[i], covered);
	};
	rec(0, 0, 0);
	return best;
}
const SKIP = 1e6;
function carry(x, y, from, to, theta, s) {
	const c = Math.cos(theta) * s;
	const n = Math.sin(theta) * s;
	const dx = x - from[0];
	const dy = y - from[1];
	return [to[0] + dx * c - dy * n, to[1] + dx * n + dy * c];
}
function dots(c, n) {
	const d = new Float64Array(2 * n);
	for (let i = 0; i < 2 * n; i += 2) {
		d[i] = c[0];
		d[i + 1] = c[1];
	}
	return d;
}
function inFrame(a, b, f, hole, half) {
	const n = a.length / 2;
	const s = Math.exp(f.lnSigma);
	const aC = new Float64Array(2 * n);
	const bt = new Float64Array(2 * n);
	let nb = 0;
	for (let i = 0; i < 2 * n; i += 2) {
		aC[i] = a[i] - f.ca[0];
		aC[i + 1] = a[i + 1] - f.ca[1];
		[bt[i], bt[i + 1]] = carry(b[i], b[i + 1], f.cb, [0, 0], -f.theta, 1 / s);
		nb += (b[i] - f.cb[0]) ** 2 + (b[i + 1] - f.cb[1]) ** 2;
	}
	let off = 0;
	let be = Infinity;
	for (let o = 0; o < n && be > 0; o++) {
		let e = 0;
		for (let i = 0; i < n && e < be; i++) {
			const j = (i + o) % n;
			e += (aC[2 * i] - bt[2 * j]) ** 2 + (aC[2 * i + 1] - bt[2 * j + 1]) ** 2;
		}
		if (e < be) {
			be = e;
			off = o;
		}
	}
	return {
		a,
		aC,
		bT: rotatePts(bt, off),
		bO: rotatePts(b, off),
		ca: f.ca,
		cb: f.cb,
		theta: f.theta,
		lnSigma: f.lnSigma,
		res: nb > 1e-12 ? Math.sqrt(s * s * be / nb) : 0,
		closed: true,
		hole,
		half,
		block: f.block
	};
}
function vanishingHole(pts, outer, grow) {
	const s = Math.exp(outer.lnSigma);
	let [x, y] = centroid(pts);
	const { aC, bT } = outer;
	const near = grow ? outer.bO : outer.a;
	let w = 0;
	let dx = 0;
	let dy = 0;
	for (let i = 0; i < aC.length; i += 2) {
		const k = 1 / ((near[i] - x) ** 2 + (near[i + 1] - y) ** 2 + 1e-9);
		w += k;
		dx += k * (bT[i] - aC[i]);
		dy += k * (bT[i + 1] - aC[i + 1]);
	}
	w = (grow ? -.5 : .5) / w;
	if (grow) [x, y] = carry(x, y, outer.cb, outer.ca, -outer.theta, 1 / s);
	x += w * dx;
	y += w * dy;
	const dot = dots(grow ? [x, y] : carry(x, y, outer.ca, outer.cb, outer.theta, s), pts.length / 2);
	return grow ? inFrame(dot, pts, outer, true, 1) : inFrame(pts, dot, outer, true, 0);
}
const weight = (it, t) => Math.min(1, Math.max(0, 2 * t - (it.half ?? t)));
function strays(h, o, holes) {
	let k = 0;
	for (const t of [
		.25,
		.5,
		.75
	]) {
		if (!(weight(h, t) % 1)) continue;
		const [H, O, ...G] = [
			h,
			o,
			...holes.filter((g) => g !== h)
		].map((it) => it.aC.map((v, i) => v + weight(it, t) * (it.bT[i] - v)));
		for (let i = 0; i < H.length; i += 2) if (!inside(H[i], H[i + 1], O) || G.some((p) => inside(H[i], H[i + 1], p))) k++;
	}
	return k;
}
function vanishingPiece(outer, holes, grow) {
	const c = centroid(outer);
	const f = {
		ca: c,
		cb: c,
		theta: 0,
		lnSigma: 0,
		block: null
	};
	const dot = dots(c, outer.length / 2);
	return [outer, ...holes].map((p, k) => grow ? inFrame(dot, p, f, k > 0) : inFrame(p, dot, f, k > 0));
}
const vanishCost = (pts) => costMatrix([pts], [dots(centroid(pts), pts.length / 2)])[0][0];
function bbox(p) {
	const b = [
		p[0],
		p[1],
		p[0],
		p[1]
	];
	for (let i = 2; i < p.length; i += 2) {
		b[0] = Math.min(b[0], p[i]);
		b[1] = Math.min(b[1], p[i + 1]);
		b[2] = Math.max(b[2], p[i]);
		b[3] = Math.max(b[3], p[i + 1]);
	}
	return b;
}
const HOLE_IOU = .25;
function holeCosts(HA, HB, outer) {
	const s = Math.exp(outer.lnSigma);
	const C = costMatrix(HA, HB);
	const inA = HB.map((h) => {
		const q = new Float64Array(h.length);
		for (let i = 0; i < h.length; i += 2) [q[i], q[i + 1]] = carry(h[i], h[i + 1], outer.cb, outer.ca, -outer.theta, 1 / s);
		return bbox(q);
	});
	HA.forEach((h, i) => {
		const a = bbox(h);
		inA.forEach((b, j) => {
			const w = Math.min(a[2], b[2]) - Math.max(a[0], b[0]);
			const v = Math.min(a[3], b[3]) - Math.max(a[1], b[1]);
			const inter = w > 0 && v > 0 ? w * v : 0;
			const union = (a[2] - a[0]) * (a[3] - a[1]) + (b[2] - b[0]) * (b[3] - b[1]) - inter;
			if (!(inter >= HOLE_IOU * union)) C[i][j] = Infinity;
		});
	});
	return C;
}
function piecesOf(subs) {
	const idx = [];
	subs.forEach((s, k) => {
		if (!s.hole) idx.push(k);
	});
	const holes = idx.map(() => []);
	subs.forEach((s) => {
		if (!s.hole) return;
		let k = idx.indexOf(parentOf(s.pts, subs));
		if (k < 0) {
			const [hx, hy] = centroid(s.pts);
			let bd = Infinity;
			idx.forEach((o, m) => {
				const [ox, oy] = centroid(subs[o].pts);
				const d = Math.hypot(ox - hx, oy - hy);
				if (d < bd) {
					bd = d;
					k = m;
				}
			});
		}
		holes[k].push(s.pts);
	});
	return {
		outers: idx.map((k) => subs[k].pts),
		holes
	};
}
function doubled(p) {
	const m = p.length;
	const q = new Float64Array(2 * m);
	for (let i = 0; i < m; i += 2) {
		const j = (i + 2) % m;
		q[2 * i] = p[i];
		q[2 * i + 1] = p[i + 1];
		q[2 * i + 2] = (p[i] + p[j]) / 2;
		q[2 * i + 3] = (p[i + 1] + p[j + 1]) / 2;
	}
	return q;
}
/** Region plan: outers ↔ outers (holed contours never duplicated), holes ↔
*  overlapping holes of the paired outer, direction locked; whatever has no
*  partner collapses/grows without ever breaking the nesting (rules above).
*  Every item flies closed. */
function buildRegionPlan(srcSubs, dstSubs) {
	const n = Math.max(srcSubs[0].pts.length, dstSubs[0].pts.length) / 2;
	const fit = (s) => s[0].pts.length / 2 < n ? s.map((c) => ({
		...c,
		pts: doubled(c.pts)
	})) : s;
	const A = piecesOf(fit(srcSubs));
	const B = piecesOf(fit(dstSubs));
	if (A.outers.length === 0 || B.outers.length === 0) throw new Error("morphicons: region has no outer contour");
	let pairs;
	const p = A.outers.length;
	const q = B.outers.length;
	const [large, small] = p < q ? [B, A] : [A, B];
	if (p === q || small.holes.every((h) => h.length === 0)) pairs = pairUp(A.outers, B.outers);
	else pairs = assign(costMatrix(large.outers, small.outers), large.outers.map((o) => SKIP + vanishCost(o)), small.holes.map((h) => h.length > 0), true).map((s, l) => p < q ? [s, l] : [l, s]);
	const items = [];
	let partial = false;
	for (const [i, j] of pairs) {
		if (i < 0 || j < 0) {
			partial = true;
			items.push(...i < 0 ? vanishingPiece(B.outers[j], B.holes[j], true) : vanishingPiece(A.outers[i], A.holes[i], false));
			continue;
		}
		const it = outerItem(A.outers[i], B.outers[j]);
		it.hole = false;
		items.push(it);
		const HA = A.holes[i];
		const HB = B.holes[j];
		const f = assign(holeCosts(HA, HB, it), HA.map(() => SKIP), HB.map(() => true), false);
		const holes = HA.map((h, k) => f[k] < 0 ? vanishingHole(h, it, false) : inFrame(h, HB[f[k]], it, true));
		HB.forEach((h, k) => {
			if (!f.includes(k)) holes.push(vanishingHole(h, it, true));
		});
		for (let k = 0; k < holes.length; k++) {
			const h = holes[k];
			if (h.half !== void 0) {
				partial = true;
				continue;
			}
			const c = strays(h, it, holes);
			if (!c) continue;
			holes.splice(k, 1, vanishingHole(h.a, it, false), vanishingHole(h.bO, it, true));
			if (strays(holes[k], it, holes) + strays(holes[k + 1], it, holes) >= c) holes.splice(k, 2, h);
		}
		items.push(...holes);
	}
	if (items.length > 1 && !partial) applyGlobal(items, n);
	return {
		items,
		n
	};
}
//#endregion
//#region src/region/index.ts
const BIG = 1e9;
const painted = (v) => v !== void 0 && String(v).trim().toLowerCase() !== "none";
const num = (v, fallback) => {
	const x = typeof v === "number" ? v : Number.parseFloat(String(v ?? ""));
	return Number.isFinite(x) ? x : fallback;
};
function flatten(input, tol) {
	const lines = [];
	const closed = [];
	for (const { pts, closed: c } of iconToCubics(input)) {
		const out = [pts[0], pts[1]];
		for (let i = 0; i + 7 < pts.length; i += 6) {
			const x0 = pts[i];
			const y0 = pts[i + 1];
			const x1 = pts[i + 2];
			const y1 = pts[i + 3];
			const x2 = pts[i + 4];
			const y2 = pts[i + 5];
			const x3 = pts[i + 6];
			const y3 = pts[i + 7];
			const dd = Math.max(Math.hypot(x0 - 2 * x1 + x2, y0 - 2 * y1 + y2), Math.hypot(x1 - 2 * x2 + x3, y1 - 2 * y2 + y3));
			const k = Math.min(128, Math.max(1, Math.ceil(Math.sqrt(6 * dd / (8 * tol)))));
			for (let s = 1; s <= k; s++) {
				const t = s / k;
				const u = 1 - t;
				out.push(u * u * u * x0 + 3 * u * u * t * x1 + 3 * u * t * t * x2 + t * t * t * x3, u * u * u * y0 + 3 * u * u * t * y1 + 3 * u * t * t * y2 + t * t * t * y3);
			}
		}
		lines.push(Float64Array.from(out));
		closed.push(c);
	}
	return {
		lines,
		closed
	};
}
function partsOf(input, width, tol) {
	if (typeof input === "string") return [{
		...flatten(input, tol),
		half: width / 2,
		fill: 0
	}];
	const fillIcon = paintOf(input) === "fill";
	const parts = [];
	for (const el of input) {
		const a = el[1];
		if (translucent(a)) continue;
		const strokeOn = fillIcon ? painted(a.stroke) : a.stroke === void 0 || painted(a.stroke);
		const fillOn = fillIcon && (painted(a.fill) || a.fill === void 0 && !strokeOn);
		if (!strokeOn && !fillOn) continue;
		parts.push({
			...flatten([el], tol),
			half: strokeOn ? num(a["stroke-width"], width) / 2 : 0,
			fill: fillOn ? String(a["fill-rule"] ?? "").trim() === "evenodd" ? 2 : 1 : 0
		});
	}
	return parts;
}
function eachNear(g, ax, ay, bx, by, r, fn) {
	const i0 = Math.max(0, Math.floor((Math.min(ax, bx) - r - g.x0) / g.h));
	const i1 = Math.min(g.nx - 1, Math.ceil((Math.max(ax, bx) + r - g.x0) / g.h));
	const j0 = Math.max(0, Math.floor((Math.min(ay, by) - r - g.y0) / g.h));
	const j1 = Math.min(g.ny - 1, Math.ceil((Math.max(ay, by) + r - g.y0) / g.h));
	for (let j = j0; j <= j1; j++) {
		const y = g.y0 + j * g.h;
		for (let i = i0; i <= i1; i++) fn(j * g.nx + i, g.x0 + i * g.h, y);
	}
}
function eachSeg(p, close, fn) {
	const n = p.length / 2;
	if (n === 1) fn(p[0], p[1], p[0], p[1]);
	for (let i = 0; i + 1 < n; i++) fn(p[2 * i], p[2 * i + 1], p[2 * i + 2], p[2 * i + 3]);
	if (close && n > 2) fn(p[2 * n - 2], p[2 * n - 1], p[0], p[1]);
}
function field(parts, g) {
	const F = new Float32Array(g.nx * g.ny).fill(BIG);
	const band = 2 * g.h;
	let D = null;
	for (const part of parts) {
		if (part.half > 0) {
			const r = part.half + band;
			part.lines.forEach((p, li) => {
				eachSeg(p, part.closed[li], (ax, ay, bx, by) => eachNear(g, ax, ay, bx, by, r, (k, x, y) => {
					const d = distSeg(x, y, ax, ay, bx, by) - part.half;
					if (d < F[k]) F[k] = d;
				}));
			});
		}
		if (part.fill) {
			D ??= new Float32Array(g.nx * g.ny);
			const Dm = D;
			Dm.fill(BIG);
			for (const p of part.lines) eachSeg(p, true, (ax, ay, bx, by) => eachNear(g, ax, ay, bx, by, band, (k, x, y) => {
				const d = distSeg(x, y, ax, ay, bx, by);
				if (d < Dm[k]) Dm[k] = d;
			}));
			const xs = [];
			const ws = [];
			for (let j = 0; j < g.ny; j++) {
				const y = g.y0 + j * g.h;
				const cross = [];
				for (const p of part.lines) eachSeg(p, true, (ax, ay, bx, by) => {
					if (ay <= y !== by <= y) cross.push([ax + (y - ay) * (bx - ax) / (by - ay), by > ay ? 1 : -1]);
				});
				cross.sort((u, v) => u[0] - v[0]);
				xs.length = 0;
				ws.length = 0;
				for (const [x, w] of cross) {
					xs.push(x);
					ws.push(w);
				}
				let c = 0;
				let wind = 0;
				for (let i = 0; i < g.nx; i++) {
					const x = g.x0 + i * g.h;
					while (c < xs.length && xs[c] < x) wind += ws[c++];
					const isIn = part.fill === 1 ? wind !== 0 : (wind & 1) === 1;
					const k = j * g.nx + i;
					const v = isIn ? -Dm[k] : Dm[k];
					if (v < F[k]) F[k] = v;
				}
			}
		}
	}
	return F;
}
function trace(F, g) {
	const { nx, ny, h, x0, y0 } = g;
	const pt = /* @__PURE__ */ new Map();
	const crossing = (key) => {
		if (pt.has(key)) return key;
		const node = key >> 1;
		const i = node % nx;
		const j = (node - i) / nx;
		const other = key & 1 ? node + nx : node + 1;
		const fa = F[node];
		const t = Math.min(Math.max(fa / (fa - F[other]), 1e-5), .99999);
		pt.set(key, key & 1 ? [x0 + i * h, y0 + (j + t) * h] : [x0 + (i + t) * h, y0 + j * h]);
		return key;
	};
	const adj = /* @__PURE__ */ new Map();
	const link = (a, b) => {
		const ka = crossing(a);
		const kb = crossing(b);
		const la = adj.get(ka);
		if (la) la.push(kb);
		else adj.set(ka, [kb]);
		const lb = adj.get(kb);
		if (lb) lb.push(ka);
		else adj.set(kb, [ka]);
	};
	for (let j = 0; j + 1 < ny; j++) for (let i = 0; i + 1 < nx; i++) {
		const a = j * nx + i;
		const b = a + 1;
		const c = a + nx + 1;
		const d = a + nx;
		const ia = F[a] < 0;
		const ib = F[b] < 0;
		const ic = F[c] < 0;
		const id = F[d] < 0;
		const top = 2 * a;
		const right = 2 * b + 1;
		const bottom = 2 * d;
		const left = 2 * a + 1;
		const edges = [];
		if (ia !== ib) edges.push(top);
		if (ib !== ic) edges.push(right);
		if (id !== ic) edges.push(bottom);
		if (ia !== id) edges.push(left);
		if (edges.length === 2) link(edges[0], edges[1]);
		else if (edges.length === 4) {
			if ((F[a] + F[b] + F[c] + F[d]) / 4 < 0 === ia) {
				link(top, right);
				link(bottom, left);
			} else {
				link(left, top);
				link(right, bottom);
			}
		}
	}
	const loops = [];
	const seen = /* @__PURE__ */ new Set();
	for (const start of adj.keys()) {
		if (seen.has(start)) continue;
		const ring = [];
		let prev = -1;
		let cur = start;
		while (!seen.has(cur)) {
			seen.add(cur);
			const p = pt.get(cur);
			ring.push(p[0], p[1]);
			const nb = adj.get(cur);
			const next = nb[0] !== prev ? nb[0] : nb[1];
			prev = cur;
			cur = next;
		}
		if (ring.length >= 6) loops.push(Float64Array.from(ring));
	}
	return loops;
}
function rdp(p, i0, i1, tol, keep) {
	const n = p.length / 2;
	const ax = p[2 * (i0 % n)];
	const ay = p[2 * (i0 % n) + 1];
	const bx = p[2 * (i1 % n)];
	const by = p[2 * (i1 % n) + 1];
	let worst = -1;
	let wd = tol;
	for (let i = i0 + 1; i < i1; i++) {
		const d = distSeg(p[2 * (i % n)], p[2 * (i % n) + 1], ax, ay, bx, by);
		if (d > wd) {
			wd = d;
			worst = i;
		}
	}
	if (worst < 0) return;
	keep[worst % n] = 1;
	rdp(p, i0, worst, tol, keep);
	rdp(p, worst, i1, tol, keep);
}
function simplify(p, tol) {
	const n = p.length / 2;
	if (n < 4) return p;
	let far = 0;
	let fd = -1;
	for (let i = 1; i < n; i++) {
		const d = Math.hypot(p[2 * i] - p[0], p[2 * i + 1] - p[1]);
		if (d > fd) {
			fd = d;
			far = i;
		}
	}
	const keep = new Uint8Array(n);
	keep[0] = 1;
	keep[far] = 1;
	rdp(p, 0, far, tol, keep);
	rdp(p, far, n, tol, keep);
	const out = [];
	for (let i = 0; i < n; i++) if (keep[i]) out.push(p[2 * i], p[2 * i + 1]);
	return Float64Array.from(out);
}
function recoverCorners(p, h) {
	const n = p.length / 2;
	if (n < 4) return p;
	const len = (k) => {
		const a = k % n;
		const b = (k + 1) % n;
		return Math.hypot(p[2 * b] - p[2 * a], p[2 * b + 1] - p[2 * a + 1]);
	};
	const long = [];
	for (let k = 0; k < n; k++) if (len(k) >= 2.5 * h) long.push(k);
	if (long.length < 2) return p;
	const drop = new Uint8Array(n);
	const moved = /* @__PURE__ */ new Map();
	for (let q = 0; q < long.length; q++) {
		const k1 = long[q];
		const k2 = long[(q + 1) % long.length];
		const gap = (k2 - k1 - 1 + n) % n;
		if (gap < 1 || gap > 3) continue;
		let gl = 0;
		for (let s = 1; s <= gap; s++) gl += len(k1 + s);
		if (gl > 3 * h) continue;
		const a0 = k1 % n;
		const a1 = (k1 + 1) % n;
		const b0 = k2 % n;
		const b1 = (k2 + 1) % n;
		const d1x = p[2 * a1] - p[2 * a0];
		const d1y = p[2 * a1 + 1] - p[2 * a0 + 1];
		const d2x = p[2 * b1] - p[2 * b0];
		const d2y = p[2 * b1 + 1] - p[2 * b0 + 1];
		const cr = d1x * d2y - d1y * d2x;
		if (Math.abs(Math.atan2(cr, d1x * d2x + d1y * d2y)) < Math.PI / 12) continue;
		const t = ((p[2 * b0] - p[2 * a0]) * d2y - (p[2 * b0 + 1] - p[2 * a0 + 1]) * d2x) / cr;
		const X = [p[2 * a0] + t * d1x, p[2 * a0 + 1] + t * d1y];
		const mx = (p[2 * a1] + p[2 * b0]) / 2;
		const my = (p[2 * a1 + 1] + p[2 * b0 + 1]) / 2;
		if (Math.hypot(X[0] - mx, X[1] - my) > 2 * h) continue;
		moved.set(a1, X);
		for (let s = 2; s <= gap + 1; s++) drop[(k1 + s) % n] = 1;
	}
	if (moved.size === 0) return p;
	const out = [];
	for (let i = 0; i < n; i++) {
		if (drop[i]) continue;
		const m = moved.get(i);
		if (m) out.push(m[0], m[1]);
		else out.push(p[2 * i], p[2 * i + 1]);
	}
	return Float64Array.from(out);
}
const fmt = (v) => String(Math.round(v * 1e3) / 1e3);
/** Contours of the visible surface: closed, non-overlapping, outers
*  positive / holes negative (nesting depth decides which is which). */
function regionContours(input, options) {
	const h = options?.cell ?? .125;
	const parts = partsOf(input, options?.strokeWidth ?? 2, .004);
	let minX = Number.POSITIVE_INFINITY;
	let minY = Number.POSITIVE_INFINITY;
	let maxX = Number.NEGATIVE_INFINITY;
	let maxY = Number.NEGATIVE_INFINITY;
	let pad = 0;
	for (const part of parts) {
		pad = Math.max(pad, part.half);
		for (const p of part.lines) for (let i = 0; i < p.length; i += 2) {
			minX = Math.min(minX, p[i]);
			maxX = Math.max(maxX, p[i]);
			minY = Math.min(minY, p[i + 1]);
			maxY = Math.max(maxY, p[i + 1]);
		}
	}
	if (!(maxX >= minX)) return [];
	pad += 3 * h;
	const g = {
		x0: minX - pad,
		y0: minY - pad,
		h,
		nx: Math.ceil((maxX - minX + 2 * pad) / h) + 1,
		ny: Math.ceil((maxY - minY + 2 * pad) / h) + 1
	};
	const loops = trace(field(parts, g), g).filter((l) => Math.abs(signedArea(l)) > .01);
	return loops.map((l) => {
		let depth = 0;
		for (const o of loops) if (o !== l && inside(l[0], l[1], o)) depth++;
		const hole = depth % 2 === 1;
		return recoverCorners(simplify(signedArea(l) > 0 === !hole ? l : reversePts(l), h * .08), h);
	});
}
/** Any icon → its canonical region: one `<path>` (nonzero) whose contours
*  are the visible surface. Idempotent on regions. Stroke icons outline at
*  `strokeWidth` (round caps and joins — Lucide, Tabler, Feather,
*  Heroicons outline); fill icons honor each element's fill rule. */
function toRegion(input, options) {
	if (isRegion(input)) return input;
	let d = "";
	for (const p of regionContours(input, options)) {
		d += `M${fmt(p[0])} ${fmt(p[1])}`;
		for (let i = 2; i < p.length; i += 2) d += `L${fmt(p[i])} ${fmt(p[i + 1])}`;
		d += "Z";
	}
	if (!d) throw new Error("morphicons: icon has no visible surface");
	return [["path", {
		d,
		fill: "currentColor",
		stroke: "none",
		"fill-rule": "nonzero",
		[REGION_ATTR]: "1"
	}]];
}
const forms = /* @__PURE__ */ new WeakMap();
const byContent = /* @__PURE__ */ new Map();
const samples = /* @__PURE__ */ new WeakMap();
const plans = /* @__PURE__ */ new WeakMap();
/** The region the driver draws for an icon (cached): a stroke icon
*  outlined at the element's stroke width, a fill icon as is. */
function regionOf(icon, strokeWidth) {
	if (isRegion(icon)) return icon;
	const width = paintOf(icon) === "fill" && !icon.some(([, a]) => painted(a.stroke) && a["stroke-width"] === void 0) ? 0 : strokeWidth;
	let byWidth = typeof icon === "string" ? void 0 : forms.get(icon);
	let r = byWidth?.get(width);
	if (r) return r;
	const key = `${width}|${typeof icon === "string" ? icon : JSON.stringify(icon)}`;
	r = byContent.get(key);
	if (r) byContent.delete(key);
	else r = toRegion(icon, { strokeWidth });
	byContent.set(key, r);
	if (byContent.size > 64) byContent.delete(byContent.keys().next().value);
	if (typeof icon !== "string") {
		if (!byWidth) {
			byWidth = /* @__PURE__ */ new Map();
			forms.set(icon, byWidth);
		}
		byWidth.set(width, r);
	}
	return r;
}
function sampledOf(region) {
	let s = samples.get(region);
	if (!s) {
		s = resampleRegion(region);
		samples.set(region, s);
	}
	return s;
}
/** Region plan from `src` at rest to `dst` (cached by reference). */
function regionPlan(src, dst, strokeWidth) {
	const a = regionOf(src, strokeWidth);
	const b = regionOf(dst, strokeWidth);
	let inner = plans.get(a);
	if (!inner) {
		inner = /* @__PURE__ */ new WeakMap();
		plans.set(a, inner);
	}
	let p = inner.get(b);
	if (!p) {
		p = buildRegionPlan(sampledOf(a), sampledOf(b));
		inner.set(b, p);
	}
	return p;
}
/** Region plan from the shape in flight to `dst` (never cached): region
*  samples as they are, or a centerline `d` (a stroke morph interrupted
*  toward a fill icon), outlined as it is. */
function regionPlanFrom(shape, dst, strokeWidth) {
	return buildRegionPlan(typeof shape === "string" ? resampleRegion(regionOf(shape, strokeWidth)) : shape, sampledOf(regionOf(dst, strokeWidth)));
}
globalThis[Symbol.for("morphicons.region")] = {
	regionOf,
	regionPlan,
	regionPlanFrom
};
//#endregion
export { buildRegionPlan, isRegion, paintOf, regionContours, regionOf, regionPlan, regionPlanFrom, resampleRegion, toRegion };
