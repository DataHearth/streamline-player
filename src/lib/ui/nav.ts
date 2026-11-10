import { goto as kitGoto } from "$app/navigation";
import { asset, resolve } from "$app/paths";

const prefix = () => resolve("/").replace(/\/$/, "");

/** `goto` for app paths such as `/detail/movies/3`: the base path is added here, as Kit 3 does not. */
export const goto = (path: string, opts?: Parameters<typeof kitGoto>[1]) =>
	kitGoto(`${prefix()}${path}`, opts);

/** `location.pathname` without the base path, as the routes spell it. */
export const appPath = () => {
	const base = prefix();
	const { pathname } = location;
	return pathname.startsWith(`${base}/`) || pathname === base
		? pathname.slice(base.length) || "/"
		: pathname;
};

export const staticUrl = (file: Parameters<typeof asset>[0]) => asset(file);
