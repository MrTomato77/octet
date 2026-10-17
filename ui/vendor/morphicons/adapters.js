import { n as iconToCubics, r as cubicsToPathD, t as fitIcon } from "./normalize-CYnN3Npw.js";
//#region src/adapters/canvas.ts
/** Adapts a canvas (or its 2D context) into a `PathEl` whose writes stroke
*  the morph's geometry onto the canvas, so
*  `createMorph(canvasTarget(canvas), icon)` animates pixels instead of an
*  inline `<svg><path>`. */
function canvasTarget(surface, opts = {}) {
	if (typeof Path2D === "undefined") throw new Error("morphicons: canvasTarget needs Path2D (browser or worker)");
	const ctx = "getContext" in surface ? surface.getContext("2d") : surface;
	if (!ctx) throw new Error("morphicons: canvasTarget needs a 2D context");
	const vb = (opts.viewBox ?? "0 0 24 24").trim().split(/[\s,]+/).map(Number);
	const [mx, my, vw, vh] = [
		vb[0] || 0,
		vb[1] || 0,
		vb[2] || 24,
		vb[3] || vb[2] || 24
	];
	const stroke = opts.strokeWidth ?? 2;
	const clear = opts.clear !== false;
	let color = opts.color;
	let cssPending = color === void 0 && "style" in ctx.canvas;
	return { setAttribute(name, value) {
		if (name !== "d") return;
		if (cssPending) {
			cssPending = false;
			color = getComputedStyle?.(ctx.canvas).color || void 0;
		}
		const c = ctx.canvas;
		const s = Math.min(c.width / vw, c.height / vh);
		if (clear) {
			ctx.setTransform(1, 0, 0, 1, 0, 0);
			ctx.clearRect(0, 0, c.width, c.height);
		}
		ctx.setTransform(s, 0, 0, s, (c.width - vw * s) / 2 - mx * s, (c.height - vh * s) / 2 - my * s);
		ctx.lineWidth = stroke;
		ctx.lineCap = "round";
		ctx.lineJoin = "round";
		if (color !== void 0) ctx.strokeStyle = color;
		ctx.stroke(new Path2D(value));
		opts.onWrite?.();
	} };
}
//#endregion
//#region src/adapters/mask.ts
const SVG_NS = "http://www.w3.org/2000/svg";
let seq = 0;
let host = null;
/** Adapts an element into a `PathEl` whose writes drive a referenced SVG
*  mask, so `createMorph(maskTarget(el), icon)` animates a mask-styled
*  element instead of an inline `<svg><path>`.
*
*  DOUBLE BUFFER, on purpose: WebKit does not reliably repaint a masked
*  element when the CONTENT of its referenced `<mask>` mutates — a lone mask
*  whose inner path `d` changes per frame renders frozen in Safari until the
*  flight ends. So the target keeps TWO masks and every `d` write lands on
*  the back buffer and re-points `mask-image` at it: the property VALUE flips
*  (`url(#a)` ↔ `url(#b)`) each frame, which forces reference re-resolution
*  in every engine. Chromium/Gecko pay nothing extra for it. */
function maskTarget(el, opts = {}) {
	if (typeof document === "undefined") throw new Error("morphicons: maskTarget needs a DOM (client-side only)");
	const doc = document;
	if (!host) {
		host = doc.createElementNS(SVG_NS, "svg");
		host.setAttribute("width", "0");
		host.setAttribute("height", "0");
		host.setAttribute("aria-hidden", "true");
		host.setAttribute("style", "position:absolute");
		doc.body.appendChild(host);
	}
	const vb = (opts.viewBox ?? "0 0 24 24").trim().split(/[\s,]+/).map(Number);
	const [mx, my, w, h] = [
		vb[0] || 0,
		vb[1] || 0,
		vb[2] || 24,
		vb[3] || vb[2] || 24
	];
	const makeLayer = () => {
		const id = `morphicons-mask-${++seq}`;
		const mask = doc.createElementNS(SVG_NS, "mask");
		mask.setAttribute("id", id);
		mask.setAttribute("maskContentUnits", "objectBoundingBox");
		const g = doc.createElementNS(SVG_NS, "g");
		g.setAttribute("transform", `scale(${1 / w} ${1 / h}) translate(${-mx} ${-my})`);
		const path = doc.createElementNS(SVG_NS, "path");
		path.setAttribute("fill", "none");
		path.setAttribute("stroke", "#fff");
		path.setAttribute("stroke-width", String(opts.strokeWidth ?? 2));
		path.setAttribute("stroke-linecap", "round");
		path.setAttribute("stroke-linejoin", "round");
		g.appendChild(path);
		mask.appendChild(g);
		if (host) host.appendChild(mask);
		return {
			url: `url(#${id})`,
			mask,
			path
		};
	};
	const layers = [makeLayer(), makeLayer()];
	let front = 0;
	const s = el.style;
	s.maskImage = s.webkitMaskImage = layers[0].url;
	if (opts.paint !== false) s.backgroundColor = "currentColor";
	return {
		setAttribute(name, d) {
			if (name !== "d") return;
			front = 1 - front;
			const layer = layers[front];
			layer.path.setAttribute("d", d);
			s.maskImage = s.webkitMaskImage = layer.url;
		},
		dispose() {
			layers[0].mask.remove();
			layers[1].mask.remove();
		}
	};
}
//#endregion
//#region src/adapters/svg.ts
const SVG_CONTAINER = /<(defs|mask|clipPath|symbol)\b[^>]*>[\s\S]*?<\/\1>/gi;
const SVG_TAG = /<(\/?)([a-zA-Z][\w:-]*)((?:[^>"']|"[^"]*"|'[^']*')*?)(\/?)>/g;
const SVG_ATTR = /([\w:-]+)\s*=\s*"([^"]*)"|([\w:-]+)\s*=\s*'([^']*)'/g;
const SVG_VIEWBOX = /<svg\b[^>]*\bviewBox\s*=\s*["']([^"']+)["']/i;
const SVG_GEOMETRY = /* @__PURE__ */ new Set([
	"path",
	"line",
	"circle",
	"ellipse",
	"rect",
	"polyline",
	"polygon"
]);
const SVG_GROUP = /* @__PURE__ */ new Set([
	"svg",
	"g",
	"a"
]);
const SVG_SKIP = /* @__PURE__ */ new Set([
	"title",
	"desc",
	"defs",
	"mask",
	"clippath",
	"symbol"
]);
const PAINT = [
	"fill",
	"stroke",
	"stroke-width",
	"fill-rule",
	"opacity",
	"fill-opacity",
	"visibility",
	"display"
];
function readSvgAttrs(raw) {
	const attrs = {};
	SVG_ATTR.lastIndex = 0;
	let m;
	while ((m = SVG_ATTR.exec(raw)) !== null) {
		const name = m[1] ?? m[3];
		if (name) attrs[name] = m[2] ?? m[4] ?? "";
	}
	return attrs;
}
function paintOf(parent, attrs) {
	const out = { ...parent };
	for (const k of PAINT) if (attrs[k] !== void 0) out[k] = String(attrs[k]);
	for (const decl of String(attrs.style ?? "").split(";")) {
		const i = decl.indexOf(":");
		const k = decl.slice(0, i).trim().toLowerCase();
		if (i > 0 && PAINT.includes(k)) out[k] = decl.slice(i + 1).trim();
	}
	return out;
}
const mul = (m, n) => [
	m[0] * n[0] + m[2] * n[1],
	m[1] * n[0] + m[3] * n[1],
	m[0] * n[2] + m[2] * n[3],
	m[1] * n[2] + m[3] * n[3],
	m[0] * n[4] + m[2] * n[5] + m[4],
	m[1] * n[4] + m[3] * n[5] + m[5]
];
function parseTransform(v) {
	const s = String(v ?? "").trim();
	if (!s) return null;
	let m = [
		1,
		0,
		0,
		1,
		0,
		0
	];
	for (const [, fn, args] of s.matchAll(/(\w+)\s*\(([^)]*)\)/g)) {
		const a = args.trim().split(/[\s,]+/).map(Number);
		const rad = (a[0] ?? 0) * Math.PI / 180;
		let t;
		switch (fn) {
			case "matrix":
				t = [
					a[0],
					a[1],
					a[2],
					a[3],
					a[4],
					a[5]
				];
				break;
			case "translate":
				t = [
					1,
					0,
					0,
					1,
					a[0],
					a[1] ?? 0
				];
				break;
			case "scale":
				t = [
					a[0],
					0,
					0,
					a[1] ?? a[0],
					0,
					0
				];
				break;
			case "rotate": {
				const c = Math.cos(rad);
				const n = Math.sin(rad);
				const [cx = 0, cy = 0] = [a[1], a[2]];
				t = [
					c,
					n,
					-n,
					c,
					cx - c * cx + n * cy,
					cy - n * cx - c * cy
				];
				break;
			}
			case "skewX":
				t = [
					1,
					0,
					Math.tan(rad),
					1,
					0,
					0
				];
				break;
			case "skewY":
				t = [
					1,
					Math.tan(rad),
					0,
					1,
					0,
					0
				];
				break;
			default: throw new Error(`morphicons: unsupported transform "${fn}"`);
		}
		if (t.some((x) => !Number.isFinite(x))) throw new Error(`morphicons: invalid transform "${s}"`);
		m = mul(m, t);
	}
	return m;
}
function bake(tag, attrs, m) {
	const paths = iconToCubics([[tag, attrs]]);
	for (const { pts } of paths) for (let i = 0; i < pts.length; i += 2) {
		const x = pts[i];
		const y = pts[i + 1];
		pts[i] = m[0] * x + m[2] * y + m[4];
		pts[i + 1] = m[1] * x + m[3] * y + m[5];
	}
	return cubicsToPathD(paths);
}
function stripComments(markup) {
	let out = markup;
	let prev;
	do {
		prev = out;
		out = out.replace(/<!--[\s\S]*?-->/g, "");
	} while (out !== prev);
	const open = out.indexOf("<!--");
	return open === -1 ? out : out.slice(0, open);
}
const painted = (v) => v !== void 0 && v.trim().toLowerCase() !== "none";
/** SVG markup → an IconInput the pipeline can lower. Strips non-rendered
*  containers, rejects what it can't read (an unsupported element, empty
*  geometry) and re-grids onto 24 via the viewBox when one is present.
*  Stroke icons return a `d` (when a viewBox forced a fit) or an IconNode,
*  exactly as before; fill-drawn icons return an IconNode whose elements
*  carry their resolved paint (fill/stroke/stroke-width/fill-rule), which
*  is what makes the morph fly as a region. */
function svgToIcon(markup) {
	if (markup.trimStart().charCodeAt(0) !== 60) throw new Error("morphicons: svgToIcon expects SVG markup — a d string or IconNode is already an IconInput");
	const clean = stripComments(markup);
	if (/<style\b/i.test(clean)) throw new Error("morphicons: unsupported SVG element <style>");
	const body = clean.replace(SVG_CONTAINER, "");
	const found = [];
	const stack = [{
		paint: {},
		m: null
	}];
	SVG_TAG.lastIndex = 0;
	let mt;
	while ((mt = SVG_TAG.exec(body)) !== null) {
		const tag = mt[2].toLowerCase();
		const top = stack[stack.length - 1];
		if (mt[1]) {
			if (SVG_GROUP.has(tag) && stack.length > 1) stack.pop();
			continue;
		}
		if (SVG_SKIP.has(tag)) continue;
		const attrs = readSvgAttrs(mt[3]);
		const own = parseTransform(attrs.transform);
		const m = own ? top.m ? mul(top.m, own) : own : top.m;
		const paint = paintOf(top.paint, attrs);
		if (top.paint.display === "none") paint.display = "none";
		if (SVG_GROUP.has(tag)) {
			if (!mt[4]) stack.push({
				paint,
				m
			});
			continue;
		}
		if (!SVG_GEOMETRY.has(tag)) throw new Error(`morphicons: unsupported SVG element <${tag}>`);
		if (paint.display === "none" || /^(hidden|collapse)$/.test(paint.visibility ?? "")) continue;
		found.push({
			tag,
			attrs,
			paint,
			m
		});
	}
	if (found.length === 0) throw new Error("morphicons: no morphable geometry in SVG");
	const fills = found.map(({ paint }) => paint.fill !== void 0 ? painted(paint.fill) : !painted(paint.stroke));
	const vb = SVG_VIEWBOX.exec(clean);
	if (!fills.some(Boolean)) {
		const node = found.map(({ tag, attrs, m }) => {
			if (!m) return [tag, attrs];
			const { transform: _, ...rest } = attrs;
			return ["path", {
				...rest,
				d: bake(tag, attrs, m)
			}];
		});
		return vb ? fitIcon(node, vb[1]) : node;
	}
	let scale = 1;
	if (vb) {
		const [, , w, h] = vb[1].trim().split(/[\s,]+/).map(Number);
		scale = Math.min(24 / w, 24 / h);
	}
	return found.map(({ tag, attrs, paint, m }, i) => {
		const d = m ? bake(tag, attrs, m) : tag === "path" ? String(attrs.d ?? "") : bake(tag, attrs, [
			1,
			0,
			0,
			1,
			0,
			0
		]);
		const out = {
			d: vb ? fitIcon(d, vb[1]) : d,
			fill: fills[i] ? "currentColor" : "none",
			stroke: painted(paint.stroke) ? "currentColor" : "none"
		};
		if (painted(paint.stroke)) out["stroke-width"] = String(Number.parseFloat(paint["stroke-width"] ?? "1") * scale);
		if (paint["fill-rule"]) out["fill-rule"] = paint["fill-rule"];
		if (paint.opacity) out.opacity = paint.opacity;
		if (paint["fill-opacity"]) out["fill-opacity"] = paint["fill-opacity"];
		return ["path", out];
	});
}
//#endregion
export { canvasTarget, maskTarget, svgToIcon };
