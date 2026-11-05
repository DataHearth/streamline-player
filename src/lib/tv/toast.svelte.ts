const SHORT_MS = 2600;
const LONG_MS = 3800;
const FADE_MS = 200;

type Current = { title: string; text: string; icon: "check" | "wifi" | "cpu" | "gauge"; warning: boolean; leaving: boolean };

let current = $state<Current | null>(null);
let timer: ReturnType<typeof setTimeout> | undefined;

export const activeToast = () => current;

export function toast(title: string, { text = "", icon = "check", warning = false }: { text?: string; icon?: Current["icon"]; warning?: boolean } = {}) {
	clearTimeout(timer);
	current = { title, text, icon, warning, leaving: false };
	timer = setTimeout(() => {
		if (current) current.leaving = true;
		timer = setTimeout(() => (current = null), FADE_MS);
	}, text ? LONG_MS : SHORT_MS);
}
