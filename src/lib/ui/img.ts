const BLANK = "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg'/%3E";
/** WebKit draws a broken-image glyph over the card; a blank image keeps the card's frame and background. */
export const hideBroken = (e: Event) => ((e.currentTarget as HTMLImageElement).src = BLANK);

/** Titles without a backdrop fall back to their poster; without either, the image hides. */
export const orPoster = (poster: string, position?: string) => (e: Event) => {
	const img = e.currentTarget as HTMLImageElement;
	if (img.src === poster) return hideBroken(e);
	img.src = poster;
	img.style.objectPosition = position ?? "";
};
