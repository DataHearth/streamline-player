export type Target = "web" | "linux" | "windows" | "darwin" | "android" | "ios";
export type FormFactor = "desktop" | "tv";

export const TARGET: Target = (import.meta.env.TAURI_ENV_PLATFORM as Target | undefined) || "web";
export const IS_WEB = TARGET === "web";
export const FORM_FACTOR: FormFactor = TARGET === "android" ? "tv" : "desktop";
