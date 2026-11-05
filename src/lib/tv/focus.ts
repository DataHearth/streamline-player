type Dir = "left" | "right" | "up" | "down";

const remembered = new Map<string, string>();
const zoom = () => parseFloat(getComputedStyle(document.documentElement).getPropertyValue("--tv-zoom")) || 1;

function scopeOf(root: HTMLElement) {
	return root.querySelector<HTMLElement>("[data-tv-trap]") ?? root;
}

function focusable(root: HTMLElement) {
	return [...scopeOf(root).querySelectorAll<HTMLButtonElement>("[data-tv]")].filter((el) => {
		const r = el.getBoundingClientRect();
		return r.width > 0 && r.height > 0 && !el.disabled && !el.closest("[inert]");
	});
}

// The focused element is lifted or scaled; undo its own transform so it isn't "above" its row-mates.
function layoutRect(el: HTMLElement) {
	const r = el.getBoundingClientRect();
	const m = /matrix\(([^)]+)\)/.exec(getComputedStyle(el).transform);
	if (!m) return r;
	const [a = 1, b = 0, c = 0, d = 1, e = 0, f = 0] = (m[1] ?? "").split(",").map(Number);
	const sx = Math.hypot(a, b) || 1;
	const sy = Math.hypot(c, d) || 1;
	const k = zoom();
	const cx = (r.left + r.right) / 2 - e * k;
	const cy = (r.top + r.bottom) / 2 - f * k;
	const [w, h] = [r.width / sx, r.height / sy];
	return { left: cx - w / 2, right: cx + w / 2, top: cy - h / 2, bottom: cy + h / 2 };
}

function reveal(el: HTMLElement, root: HTMLElement) {
	const k = zoom();
	for (let p = el.parentElement; p && p !== root; p = p.parentElement) {
		if (p.hasAttribute("data-tv-noscroll")) continue;
		const cs = getComputedStyle(p);
		const canX = /(auto|scroll|hidden)/.test(cs.overflowX) && p.scrollWidth > p.clientWidth + 1;
		const canY = /(auto|scroll|hidden)/.test(cs.overflowY) && p.scrollHeight > p.clientHeight + 1;
		if (!canX && !canY) continue;
		const pr = p.getBoundingClientRect();
		const er = el.getBoundingClientRect();
		if (canX) {
			const [l, r, pad] = [(er.left - pr.left) / k, (er.right - pr.left) / k, 96];
			if (l < pad) p.scrollBy({ left: l - pad, behavior: "smooth" });
			else if (r > p.clientWidth - pad) p.scrollBy({ left: r - p.clientWidth + pad, behavior: "smooth" });
		}
		if (canY) {
			const [t, b, padT, padB] = [(er.top - pr.top) / k, (er.bottom - pr.top) / k, 220, 90];
			if (t < padT) p.scrollBy({ top: Math.max(t - padT, -p.scrollTop), behavior: "smooth" });
			else if (b > p.clientHeight - padB) p.scrollBy({ top: b - p.clientHeight + padB, behavior: "smooth" });
		}
	}
}

/** Only Back returns to the remembered card; arriving any other way starts at the screen's default. */
export const forgetFocus = () => remembered.delete(location.pathname);

export function focus(el: HTMLElement | null | undefined, root: HTMLElement) {
	if (!el) return;
	el.focus({ preventScroll: true });
	reveal(el, root);
	const fid = el.getAttribute("data-fid");
	if (fid) remembered.set(location.pathname, fid);
}

export function focusDefault(root: HTMLElement, attempt = 0) {
	const scope = scopeOf(root);
	const back = remembered.get(location.pathname);
	const el =
		(back && !scope.hasAttribute("data-tv-trap") ? scope.querySelector<HTMLElement>(`[data-fid="${CSS.escape(back)}"]`) : null) ??
		[...scope.querySelectorAll<HTMLElement>("[data-tv-default]")].find((e) => !e.closest("[data-tv-nav]") && e.getBoundingClientRect().width > 0) ??
		focusable(root).find((e) => !e.closest("[data-tv-nav]"));
	if (!el && attempt < 15) {
		setTimeout(() => focusDefault(root, attempt + 1), 120);
		return;
	}
	focus(el ?? focusable(root)[0], root);
}

export function move(key: string, root: HTMLElement) {
	const dir = key.slice(5).toLowerCase() as Dir;
	const cur = document.activeElement as HTMLElement | null;
	const els = focusable(root);
	if (!cur || !root.contains(cur) || !cur.hasAttribute("data-tv") || !els.includes(cur as HTMLButtonElement)) return focusDefault(root);
	const a = layoutRect(cur);
	const [ax, ay] = [(a.left + a.right) / 2, (a.top + a.bottom) / 2];
	const row = dir === "left" || dir === "right" ? cur.closest("[data-tv-row]") : null;
	const fromNav = !!cur.closest("[data-tv-nav]");
	let best: HTMLElement | null = null;
	let bestScore = Infinity;
	let bestNav: HTMLElement | null = null;
	let bestNavScore = Infinity;
	for (const el of els) {
		if (el === cur || (row && !row.contains(el))) continue;
		const b = layoutRect(el);
		const [bx, by] = [(b.left + b.right) / 2, (b.top + b.bottom) / 2];
		const off = Math.max(a.top, b.top) - Math.min(a.bottom, b.bottom) > 0 ? Math.abs(by - ay) : 0;
		const across = Math.abs(bx - ax) * 0.6 + Math.max(0, b.left - a.left) * 0.2;
		let main: number;
		let cross: number;
		if (dir === "right") {
			if (bx <= ax + 2 || b.left < a.left + 2) continue;
			[main, cross] = [Math.max(0, b.left - a.right), off];
		} else if (dir === "left") {
			if (bx >= ax - 2 || b.right > a.right - 2) continue;
			[main, cross] = [Math.max(0, a.left - b.right), off];
		} else if (dir === "down") {
			if (by <= ay + 2 || b.top < a.top + 2) continue;
			[main, cross] = [Math.max(0, b.top - a.bottom), across];
		} else {
			if (by >= ay - 2 || b.bottom > a.bottom - 2) continue;
			[main, cross] = [Math.max(0, a.top - b.bottom), across];
		}
		const score = main + cross * 2.2;
		if (!fromNav && el.closest("[data-tv-nav]")) {
			if (score < bestNavScore) [bestNav, bestNavScore] = [el, score];
		} else if (score < bestScore) [best, bestScore] = [el, score];
	}
	// Up from the tenth card of a row means the row above, however far it sits sideways.
	best ??= bestNav;
	if (best) focus(best, root);
}
