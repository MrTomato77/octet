//#region src/core/region.ts
/** Attribute that marks an IconNode as an already-normalized region (the
*  output of `toRegion`): one `<path>` of non-overlapping closed contours,
*  outers positive, holes negative. Normalization skips it. */
const REGION_ATTR = "data-region";
/** A paint attribute that paints (not absent, not `none`). */
const painted = (v) => v !== void 0 && String(v).trim().toLowerCase() !== "none";
/** Paint of an icon. Raw `d` strings and IconNodes that never declare a
*  painted fill are stroke icons — the historical contract, so every
*  existing input keeps its meaning. One element with a painted `fill` makes
*  the whole icon a fill icon (mixed stroke+fill icons need the region path
*  too: their silhouette is not a set of centerlines). */
function paintOf(icon) {
	if (typeof icon === "string") return "stroke";
	for (const [, a] of icon) if (painted(a.fill)) return "fill";
	return "stroke";
}
/** A duotone background layer (Phosphor: opacity 0.2) is not part of a
*  monochrome silhouette: one path can't paint it translucent, so it is
*  dropped from a fill icon rather than painted opaque. */
const translucent = (a) => Number.parseFloat(String(a.opacity)) < .5 || Number.parseFloat(String(a["fill-opacity"])) < .5;
/** The elements a fill icon actually draws: its translucent layers out. */
function solid(icon) {
	return typeof icon === "string" || paintOf(icon) === "stroke" ? icon : icon.filter(([, a]) => !translucent(a));
}
/** True when the icon is already a normalized region (no work left). */
function isRegion(icon) {
	return typeof icon !== "string" && icon.length === 1 && "data-region" in icon[0][1];
}
/** Even-odd point-in-polygon against a closed point cloud. */
function inside(px, py, p) {
	const n = p.length / 2;
	let c = false;
	for (let i = 0, j = n - 1; i < n; j = i++) {
		const yi = p[2 * i + 1];
		const yj = p[2 * j + 1];
		if (yi > py !== yj > py && px < (p[2 * j] - p[2 * i]) * (py - yi) / (yj - yi) + p[2 * i]) c = !c;
	}
	return c;
}
/** Distance from (px, py) to the segment ab. */
function distSeg(px, py, ax, ay, bx, by) {
	const dx = bx - ax;
	const dy = by - ay;
	const l2 = dx * dx + dy * dy;
	let t = l2 > 0 ? ((px - ax) * dx + (py - ay) * dy) / l2 : 0;
	t = t < 0 ? 0 : t > 1 ? 1 : t;
	const ex = px - ax - t * dx;
	const ey = py - ay - t * dy;
	return Math.sqrt(ex * ex + ey * ey);
}
/** Shoelace signed area of a closed point cloud (SVG axes: y down). */
function signedArea(p) {
	const n = p.length / 2;
	let a = 0;
	for (let i = 0, j = n - 1; i < n; j = i++) a += p[2 * j] * p[2 * i + 1] - p[2 * i] * p[2 * j + 1];
	return a / 2;
}
//#endregion
export { paintOf as a, solid as c, isRegion as i, translucent as l, distSeg as n, painted as o, inside as r, signedArea as s, REGION_ATTR as t };
