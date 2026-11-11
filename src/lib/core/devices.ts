import type { Device } from "#lib/core/types.ts";
import { m as i18n } from "#lib/paraglide/messages.js";

const ACTIVE_SECS = 300;

export function seen(d: Device, short = false) {
	if (d.streaming)
		return short ? i18n.settings_streaming_now_short() : i18n.settings_streaming_now();
	const s = (Date.now() - Date.parse(d.last_seen)) / 1000;
	if (s < ACTIVE_SECS) return short ? i18n.settings_active_now_short() : i18n.settings_active_now();
	if (s < 3600)
		return (short ? i18n.settings_seen_min_short : i18n.settings_seen_min)({
			count: Math.round(s / 60),
		});
	if (s < 86400)
		return (short ? i18n.settings_seen_h_short : i18n.settings_seen_h)({
			count: Math.round(s / 3600),
		});
	const count = Math.round(s / 86400);
	if (short)
		return (count === 1 ? i18n.settings_seen_days_one_short : i18n.settings_seen_days_other_short)({
			count,
		});
	return (count === 1 ? i18n.settings_seen_days_one : i18n.settings_seen_days_other)({ count });
}
