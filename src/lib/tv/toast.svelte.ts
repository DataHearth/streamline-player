const SHORT_MS = 2600;
const LONG_MS = 3800;
const FADE_MS = 200;

type Current = { title: string; text: string; icon: "check" | "wifi" | "cpu"; leaving: boolean };

let current = $state<Current | null>(null);
let timer: ReturnType<typeof setTimeout> | undefined;

export const activeToast = () => current;

export function toast(title: string, { text = "", icon = "check" }: { text?: string; icon?: Current["icon"] } = {}) {
	clearTimeout(timer);
	current = { title, text, icon, leaving: false };
	timer = setTimeout(() => {
		if (current) current.leaving = true;
		timer = setTimeout(() => (current = null), FADE_MS);
	}, text ? LONG_MS : SHORT_MS);
}
